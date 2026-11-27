//! Bitwarden's six URI strategies and explicitly supplied source-account settings.
use super::{WebsiteMode, WebsiteRule};
use rquickjs::{Context, Function, Module, Runtime};
use serde::{Deserialize, Serialize};
use std::{
    sync::{mpsc, OnceLock},
    time::{Duration, Instant},
};
use zeroize::Zeroizing;

const INVALID: &str = "Bitwarden 网址来源设置无效；未导入任何内容";
const DOMAIN_CODE: &str = concat!(
    "const exports={};",
    include_str!("../../vendor/tldts-7.4.2/index.cjs.min.js"),
    "\nexport function extractDomains(uris){ return JSON.stringify(uris.map(uri=>{ if(!uri || !uri.trim()) return null; uri=uri.trim(); if(uri.startsWith('data:') || uri.startsWith('about:')) return null; try { const p=exports.parse(uri,{validHosts:['localhost'],allowPrivateDomains:true}); return p && p.hostname ? (p.hostname==='localhost' || p.isIp ? p.hostname : p.domain) : null; } catch { return null; } })); }"
);

fn runtime(deadline: Instant) -> Result<(Runtime, Context), ()> {
    let runtime = Runtime::new().map_err(|_| ())?;
    runtime.set_memory_limit(32 * 1024 * 1024);
    runtime.set_max_stack_size(256 * 1024);
    runtime.set_interrupt_handler(Some(Box::new(move || Instant::now() >= deadline)));
    let context = Context::full(&runtime).map_err(|_| ())?;
    Ok((runtime, context))
}

struct DomainRequest {
    inputs: Zeroizing<Vec<String>>,
    deadline: Instant,
    reply: mpsc::SyncSender<Result<Vec<Option<String>>, ()>>,
}

fn domain_worker() -> Result<&'static mpsc::SyncSender<DomainRequest>, ()> {
    static WORKER: OnceLock<Result<mpsc::SyncSender<DomainRequest>, ()>> = OnceLock::new();
    WORKER
        .get_or_init(|| {
            let (send, receive) = mpsc::sync_channel::<DomainRequest>(8);
            let (ready, initialized) = mpsc::sync_channel(1);
            std::thread::Builder::new()
                .name("secpivot-domain-parser".into())
                .spawn(move || {
                    let engine = (|| {
                        let (runtime, context) = runtime(Instant::now() + Duration::from_secs(2))?;
                        context.with(|ctx| {
                            let (module, promise) =
                                Module::declare(ctx.clone(), "secpivot-tldts", DOMAIN_CODE)
                                    .and_then(|m| m.eval())
                                    .map_err(|_| ())?;
                            promise.finish::<()>().map_err(|_| ())?;
                            ctx.globals()
                                .set(
                                    "extractDomains",
                                    module
                                        .get::<_, Function>("extractDomains")
                                        .map_err(|_| ())?,
                                )
                                .map_err(|_| ())
                        })?;
                        Ok::<_, ()>((runtime, context))
                    })();
                    let _ = ready.send(engine.as_ref().map(|_| ()).map_err(|_| ()));
                    let Ok((runtime, context)) = engine else {
                        return;
                    };
                    for request in receive {
                        let deadline = request.deadline;
                        runtime.set_interrupt_handler(Some(Box::new(move || {
                            Instant::now() >= deadline
                        })));
                        let result = if Instant::now() >= deadline {
                            Err(())
                        } else {
                            context.with(|ctx| {
                                // Only the fixed published parser runs. Inputs are function
                                // arguments; no user source, loaders, callbacks or globals.
                                let function: Function =
                                    ctx.globals().get("extractDomains").map_err(|_| ())?;
                                let encoded: Zeroizing<String> = Zeroizing::new(
                                    function
                                        .call((request.inputs.as_slice(),))
                                        .map_err(|_| ())?,
                                );
                                let values: Vec<Option<String>> =
                                    serde_json::from_str(&encoded).map_err(|_| ())?;
                                if values.len() != request.inputs.len()
                                    || Instant::now() >= deadline
                                {
                                    return Err(());
                                }
                                Ok(values)
                            })
                        };
                        // The bundle's parse() creates a fresh result. Its retained
                        // state consists of static PSL tables, regexes and numeric
                        // lookup cursors, never URI strings. Clear pending exceptions
                        // and collect unreachable argument/results before accepting more.
                        context.with(|ctx| {
                            drop(ctx.catch());
                        });
                        runtime.run_gc();
                        let _ = request.reply.try_send(result);
                        // Owned request strings are zeroized even for abandoned replies.
                    }
                })
                .map_err(|_| ())?;
            initialized
                .recv_timeout(Duration::from_secs(3))
                .map_err(|_| ())??;
            Ok(send)
        })
        .as_ref()
        .map_err(|_| ())
}

pub(super) fn prepare_domain_engine() -> Result<(), String> {
    domain_worker()
        .map(|_| ())
        .map_err(|_| "Bitwarden 域名解析器不可用；未应用任何规则".into())
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BitwardenUriSettings {
    pub default_match: u8,
    pub equivalent_domains: Vec<Vec<String>>,
}

impl BitwardenUriSettings {
    pub fn validate(&self) -> Result<(), String> {
        if self.default_match > 5 || self.equivalent_domains.len() > 128 {
            return Err(INVALID.into());
        }
        let mut bytes = 0usize;
        for group in &self.equivalent_domains {
            if group.len() > 128 {
                return Err(INVALID.into());
            }
            for domain in group {
                bytes = bytes.checked_add(domain.len()).ok_or(INVALID)?;
                if domain.is_empty()
                    || domain.len() > 1024
                    || bytes > 32 * 1024
                    || domain.chars().any(|c| c.is_control() || c.is_whitespace())
                {
                    return Err(INVALID.into());
                }
            }
        }
        Ok(())
    }

    pub(super) fn matches(&self, rules: &[WebsiteRule], target: &str, deadline: Instant) -> bool {
        let mut site_domains = None;
        let mut site_index = 0;
        for rule in rules {
            if target.is_empty() || Instant::now() >= deadline {
                return false;
            }
            // Official LoginUriView rejects empty saved/target URI before mode dispatch.
            if rule.mode == WebsiteMode::Site {
                site_index += 1;
            }
            if rule.url.is_empty() {
                continue;
            }
            let matched = match rule.mode {
                WebsiteMode::Host => host(target).is_some_and(|h| Some(h) == host(&rule.url)),
                WebsiteMode::Prefix => target.starts_with(&rule.url),
                WebsiteMode::Exact => target == rule.url,
                WebsiteMode::Regex => super::ecma_regex::matches(&rule.url, target, deadline),
                WebsiteMode::Never => false,
                WebsiteMode::Site => {
                    let domains = site_domains.get_or_insert_with(|| {
                        let mut inputs = vec![target];
                        inputs.extend(
                            rules
                                .iter()
                                .filter(|r| r.mode == WebsiteMode::Site)
                                .map(|r| r.url.as_str()),
                        );
                        domains(&inputs, deadline)
                    });
                    domains.as_ref().ok().is_some_and(|values| {
                        let Some(saved) = &values[site_index] else {
                            return false;
                        };
                        let target_domain = values[0].as_deref();
                        let equal = target_domain
                            .is_some_and(|d| unicode_domain(d) == unicode_domain(saved))
                            || self
                                .equivalent_domains
                                .iter()
                                .filter(|g| target_domain.is_some_and(|d| g.iter().any(|s| s == d)))
                                .flatten()
                                .any(|d| unicode_domain(d) == unicode_domain(saved));
                        // Official blacklist applies only to the saved Domain strategy,
                        // and compares URL.host (including nondefault ports), not hostname.
                        equal
                            && (saved != "google.com"
                                || host(target).as_deref() != Some("script.google.com"))
                    })
                }
            };
            if matched {
                return true;
            }
        }
        false
    }
}

pub(super) fn mode(value: u8) -> Result<WebsiteMode, String> {
    match value {
        0 => Ok(WebsiteMode::Site),
        1 => Ok(WebsiteMode::Host),
        2 => Ok(WebsiteMode::Prefix),
        3 => Ok(WebsiteMode::Exact),
        4 => Ok(WebsiteMode::Regex),
        5 => Ok(WebsiteMode::Never),
        _ => Err(INVALID.into()),
    }
}

fn js_whitespace(c: char) -> bool {
    matches!(c, '\u{9}'..='\u{d}' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}')
}

fn parsed(source: &str) -> Option<url::Url> {
    let value = source.trim_matches(js_whitespace);
    let value = if value.contains("://") {
        value.to_owned()
    } else if value.contains('.') {
        format!("http://{value}")
    } else {
        return None;
    };
    url::Url::parse(&value).ok()
}

/// Canonical WHATWG host, including nondefault ports; never native v1 explicit-port identity.
fn host(source: &str) -> Option<String> {
    let url = parsed(source)?;
    url.host()?;
    Some(url[url::Position::BeforeHost..url::Position::AfterPort].to_owned())
}

/// Display/launch destination only; policy keeps every original URI byte separately.
pub(super) fn launch_url(source: &str) -> Option<String> {
    let url = parsed(source)?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return None;
    }
    Some(url.to_string())
}

fn unicode_domain(domain: &str) -> String {
    domain
        .split('.')
        .map(|label| {
            if label.len() > 4
                && label
                    .get(..4)
                    .is_some_and(|p| p.eq_ignore_ascii_case("xn--"))
                && label.len() <= 63
            {
                idna::punycode::decode_to_string(&label[4..].to_ascii_lowercase())
                    .unwrap_or_else(|| label.to_owned())
            } else {
                label.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join(".")
}

/// The exact published tldts bundle selected by the official client's lockfile.
/// Its private PSL/hostname extraction execute unchanged; inputs are function arguments.
fn domains(inputs: &[&str], deadline: Instant) -> Result<Vec<Option<String>>, ()> {
    if inputs.len() > 129 || inputs.iter().any(|s| s.len() > 4096) || Instant::now() >= deadline {
        return Err(());
    }
    let worker = domain_worker()?;
    let (reply, receive) = mpsc::sync_channel(1);
    worker
        .try_send(DomainRequest {
            inputs: Zeroizing::new(inputs.iter().map(|s| (*s).to_owned()).collect()),
            deadline,
            reply,
        })
        .map_err(|_| ())?;
    let result = receive
        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        .map_err(|_| ())?;
    if Instant::now() >= deadline {
        return Err(());
    }
    result
}

#[cfg(test)]
#[path = "bitwarden_uri_tests.rs"]
mod tests;
