//! KeePassHttp request dispatch: verifier gate, associate approval, serve.
//! Extracted from `bridge::mod.rs`.
use base64::{engine::general_purpose::STANDARD, Engine as _};
use zeroize::Zeroize;

use super::crypto::{check_verifier, encrypt_field, make_verifier, response_hmac};
use super::types::{
    decrypt_request_field, BridgeEntry, BridgeHost, BridgeLogin, BridgeRequest, BridgeResponse,
};
use super::{KEY_LEN, NONCE_LEN, PROTOCOL_VERSION};
use crate::config::PasswordGeneratorSettings;
use crate::crypto::random_bytes;

const UPPER: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const LOWER: &str = "abcdefghijklmnopqrstuvwxyz";
const DIGITS: &str = "0123456789";
const SYMBOLS: &str = "!@#$%^&*()-_=+[]{};:,.<>?";
const SIMILAR: &str = "Il1O0";
const AMBIGUOUS: &str = "{}[]()/\\'\"`~,;:.<>";
/// Handle one KeePassHttp request against `host`. `approve` is invoked once
/// for a fresh `associate` (the user's explicit consent in the desktop UI);
/// returning `false` rejects the client.
pub fn handle_request(
    request: BridgeRequest,
    host: &mut dyn BridgeHost,
    approve: impl FnOnce(&str) -> bool,
) -> BridgeResponse {
    let request_type = request.request_type.clone();
    if request_type.trim().is_empty() {
        return BridgeResponse::failure(&request_type, "缺少 RequestType");
    }
    if !host.is_open() {
        return BridgeResponse::failure(&request_type, "数据库未打开或已锁定");
    }
    if request_type == "associate" {
        return handle_associate(request, host, approve);
    }

    let id = request.id.clone().unwrap_or_default();
    let Some(mut key) = host.client_key(&id) else {
        return BridgeResponse::failure(&request_type, "未授权的浏览器客户端,请在浏览器中重新关联");
    };
    let valid = check_verifier(
        &key,
        &request.nonce,
        request.verifier.as_deref().unwrap_or_default(),
    );
    if !valid {
        key.zeroize();
        return BridgeResponse::failure(&request_type, "请求校验失败");
    }
    let response = dispatch(request_type.as_str(), &request, &key, id.as_str(), host);
    key.zeroize();
    response
}

/// `associate` adds a new client key after explicit user approval; the key is
/// then bound to `id` inside the (session-held) host state.
fn handle_associate(
    request: BridgeRequest,
    host: &mut dyn BridgeHost,
    approve: impl FnOnce(&str) -> bool,
) -> BridgeResponse {
    let request_type = request.request_type.clone();
    let Some(key_b64) = request.key.as_deref() else {
        return BridgeResponse::failure(&request_type, "关联请求缺少 Key");
    };
    let mut key = match STANDARD.decode(key_b64) {
        Ok(bytes) if bytes.len() == KEY_LEN => bytes,
        _ => return BridgeResponse::failure(&request_type, "关联密钥必须是 256 位"),
    };
    let valid = check_verifier(
        &key,
        &request.nonce,
        request.verifier.as_deref().unwrap_or_default(),
    );
    if !valid {
        key.zeroize();
        return BridgeResponse::failure(&request_type, "关联校验失败");
    }
    let id = request.id.clone().unwrap_or_else(new_client_id);
    if !approve(&id) {
        key.zeroize();
        return BridgeResponse::failure(&request_type, "已拒绝浏览器连接授权");
    }
    host.register_client(&id, key);
    // Echo the bound id under the stored key (fresh response nonce).
    let mut key = host.client_key(&id).unwrap_or_default();
    let nonce = random_bytes(NONCE_LEN);
    let nonce_b64 = STANDARD.encode(&nonce);
    let verifier_b64 = make_verifier(&key, &nonce);
    let response = BridgeResponse {
        request_type,
        success: true,
        id: Some(id),
        entries: Vec::new(),
        count: None,
        password: None,
        nonce: nonce_b64.clone(),
        verifier: verifier_b64.clone(),
        hash: host.db_hash(),
        version: PROTOCOL_VERSION.to_owned(),
        hmac: response_hmac(&key, &nonce_b64, &verifier_b64),
        error: None,
    };
    key.zeroize();
    response
}

/// Random client/approval token (base64 of 12 entropy bytes).
pub fn new_client_id() -> String {
    STANDARD.encode(random_bytes(12))
}

/// Decrypt-then-serve for the authorized request types. Runs only after the
/// verifier passed, so `key` is the shared client secret.
fn dispatch(
    request_type: &str,
    request: &BridgeRequest,
    key: &[u8],
    id: &str,
    host: &mut dyn BridgeHost,
) -> BridgeResponse {
    match request_type {
        "test-associate" => {
            let mut response = BridgeResponse::success(request_type, key, host);
            response.id = Some(id.to_owned());
            response
        }
        "get-logins" | "get-logins-count" => {
            let (url, submit_url) = match decrypt_request_fields(request, key) {
                Ok(fields) => fields,
                Err(e) => return BridgeResponse::failure(request_type, &e),
            };
            let logins = host.logins_for(url.as_deref().unwrap_or_default(), submit_url.as_deref());
            let mut response = BridgeResponse::success(request_type, key, host);
            if !logins.is_empty() {
                let iv = STANDARD
                    .decode(&response.nonce)
                    .expect("fresh response nonce is valid base64");
                response.entries = build_entries(&logins, key, &iv);
            }
            if request_type == "get-logins-count" {
                response.count = Some(logins.len());
            }
            response
        }
        "set-login" => {
            let fields = match decrypt_set_login_fields(request, key) {
                Ok(fields) => fields,
                Err(e) => return BridgeResponse::failure(request_type, &e),
            };
            let result = match fields.uuid.as_deref() {
                Some(_) => host.set_login(
                    fields.login.as_deref().unwrap_or_default(),
                    fields.password.as_deref().unwrap_or_default(),
                    fields.url.as_deref().unwrap_or_default(),
                    fields.uuid.as_deref(),
                ),
                None => host.create_login(
                    fields.login.as_deref().unwrap_or_default(),
                    fields.password.as_deref().unwrap_or_default(),
                    fields.url.as_deref().unwrap_or_default(),
                ),
            };
            match result {
                Ok(()) => {
                    let mut response = BridgeResponse::success(request_type, key, host);
                    response.count = Some(1);
                    response
                }
                Err(e) => BridgeResponse::failure(request_type, &e),
            }
        }
        "generate-password" => {
            let mut response = BridgeResponse::success(request_type, key, host);
            response.password = Some(generate_password());
            response
        }
        other => BridgeResponse::failure(other, &format!("不支持的操作: {other}")),
    }
}

fn build_entries(logins: &[BridgeLogin], key: &[u8], iv: &[u8]) -> Vec<BridgeEntry> {
    logins
        .iter()
        .map(|login| BridgeEntry {
            login: encrypt_field(key, iv, &login.login),
            password: encrypt_field(key, iv, &login.password),
            name: encrypt_field(key, iv, &login.name),
            uuid: encrypt_field(key, iv, &login.uuid),
        })
        .collect()
}

fn decrypt_request_fields(
    request: &BridgeRequest,
    key: &[u8],
) -> Result<(Option<String>, Option<String>), String> {
    let url = decrypt_request_field(key, &request.nonce, request.url.as_deref())?;
    let submit_url = decrypt_request_field(key, &request.nonce, request.submit_url.as_deref())?;
    Ok((url, submit_url))
}

struct SetLoginFields {
    login: Option<String>,
    password: Option<String>,
    url: Option<String>,
    uuid: Option<String>,
}

fn decrypt_set_login_fields(request: &BridgeRequest, key: &[u8]) -> Result<SetLoginFields, String> {
    let login = decrypt_request_field(key, &request.nonce, request.login.as_deref())?;
    let password = decrypt_request_field(key, &request.nonce, request.password.as_deref())?;
    let url = decrypt_request_field(key, &request.nonce, request.url.as_deref())?;
    let uuid = decrypt_request_field(key, &request.nonce, request.uuid.as_deref())?;
    Ok(SetLoginFields {
        login,
        password,
        url,
        uuid,
    })
}

/// Random index in `0..bound` from the OS RNG.
fn rand_index(bound: usize) -> usize {
    let mut buf = [0u8; 4];
    getrandom::getrandom(&mut buf).expect("OS RNG must be available");
    u32::from_le_bytes(buf) as usize % bound
}

/// Fresh 20-char password over upper/lower/digits/symbols, one character from
/// each category guaranteed — mirrors the app's default generator settings
/// (`DEFAULT_DATABASE_SETTINGS.generator`). Shared with the KeePassRPC bridge.
pub(crate) fn generate_password() -> String {
    generate_password_with(&PasswordGeneratorSettings::default())
        .expect("default generator settings are valid")
}

/// Same rule engine as the renderer `generatePassword` (custom charset,
/// exclusions, required chars, and `u/l/d/s/a` patterns). Randomness always
/// comes from the OS RNG; an empty pool falls back to upper+lower+digits.
pub(crate) fn generate_password_with(
    settings: &PasswordGeneratorSettings,
) -> Result<String, String> {
    let pool = build_pool(settings);
    let required = unique_chars(settings.required_chars.as_deref().unwrap_or(""));
    for required_char in &required {
        if !pool.contains(required_char) {
            return Err(format!("必含字符 {required_char} 不在字符池中"));
        }
    }

    if let Some(pattern) = settings.pattern.as_deref() {
        if !pattern.is_empty() {
            let mut out: Vec<char> = pattern
                .chars()
                .map(|code| match code {
                    'u' | 'l' | 'd' | 's' => {
                        let category = category_chars(code, settings);
                        if category.is_empty() {
                            pick(&pool)
                        } else {
                            pick(&category)
                        }
                    }
                    'a' => pick(&pool),
                    literal => literal,
                })
                .collect();
            guarantee_required(&mut out, &required);
            return Ok(out.into_iter().collect());
        }
    }

    let len = usize::try_from(settings.length.max(1)).unwrap_or(20);
    let mut out: Vec<char> = (0..len).map(|_| pick(&pool)).collect();
    if settings.custom_charset.as_deref().unwrap_or("").is_empty() {
        for (category, enabled) in [
            (UPPER, settings.include_upper),
            (LOWER, settings.include_lower),
            (DIGITS, settings.include_digits),
        ] {
            if enabled && !out.iter().any(|c| category.contains(*c)) {
                let category_chars = category_chars_for(category, settings);
                if !category_chars.is_empty() {
                    let pos = rand_index(out.len());
                    out[pos] = pick(&category_chars);
                }
            }
        }
    }
    guarantee_required(&mut out, &required);
    Ok(out.into_iter().collect())
}

fn build_pool(settings: &PasswordGeneratorSettings) -> Vec<char> {
    let mut pool: Vec<char> = if let Some(custom) = settings.custom_charset.as_deref() {
        if !custom.is_empty() {
            custom.chars().collect()
        } else {
            format!("{UPPER}{LOWER}{DIGITS}{SYMBOLS}").chars().collect()
        }
    } else {
        format!("{UPPER}{LOWER}{DIGITS}{SYMBOLS}").chars().collect()
    };
    if settings.exclude_similar {
        pool.retain(|c| !SIMILAR.contains(*c));
    }
    if settings.exclude_ambiguous {
        pool.retain(|c| !AMBIGUOUS.contains(*c));
    }
    if let Some(excluded) = settings.exclude_chars.as_deref() {
        let excluded: Vec<char> = excluded.chars().collect();
        pool.retain(|c| !excluded.contains(c));
    }
    if pool.is_empty() {
        pool = format!("{UPPER}{LOWER}{DIGITS}").chars().collect();
    }
    pool
}

fn category_chars_for(category: &str, settings: &PasswordGeneratorSettings) -> Vec<char> {
    let mut chars: Vec<char> = category.chars().collect();
    if settings.exclude_similar {
        chars.retain(|c| !SIMILAR.contains(*c));
    }
    if settings.exclude_ambiguous {
        chars.retain(|c| !AMBIGUOUS.contains(*c));
    }
    if let Some(excluded) = settings.exclude_chars.as_deref() {
        let excluded: Vec<char> = excluded.chars().collect();
        chars.retain(|c| !excluded.contains(c));
    }
    chars
}

fn category_chars(code: char, settings: &PasswordGeneratorSettings) -> Vec<char> {
    let category = match code {
        'u' => UPPER,
        'l' => LOWER,
        'd' => DIGITS,
        's' => SYMBOLS,
        _ => "",
    };
    category_chars_for(category, settings)
}

fn unique_chars(value: &str) -> Vec<char> {
    let mut seen = Vec::new();
    for c in value.chars() {
        if !seen.contains(&c) {
            seen.push(c);
        }
    }
    seen
}

fn guarantee_required(out: &mut [char], required: &[char]) {
    for required_char in required {
        if out.contains(required_char) {
            continue;
        }
        out[rand_index(out.len())] = *required_char;
    }
}

fn pick(pool: &[char]) -> char {
    pool[rand_index(pool.len())]
}
