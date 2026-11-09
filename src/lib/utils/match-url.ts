/** Compute a "shortest matchable address" for a KeePassRPC per-entry rule,
 * mirroring the backend's 3-tier match (`Exact`/`Hostname`/`Domain`).
 *
 * Given the real URL Kee reported (e.g. `FindLogins urls=[...]`), return the
 * shortest string that still matches under the currently selected accuracy:
 * - `Exact`: the complete original URL, including scheme/query/fragment and case.
 * - `Hostname`: `host[:port]` only (path dropped).
 * - `Domain`: the host, kept scoped so only the host and its subdomains match
 *   (e.g. `passport.aliyun.com` covers `passport.aliyun.com` and deeper hosts,
 *   but never spreads to the bare registrable domain `aliyun.com`).
 */

type Accuracy = "Exact" | "Hostname" | "Domain";

/** Validate authority with WHATWG URL, retaining an original explicit port
 * that the parser otherwise removes when it equals the scheme's default. */
function parseAuthority(value: string): { host: string; port: number | null } | null {
  const input = value.replace(/^\p{White_Space}+|\p{White_Space}+$/gu, "");
  if (!input || /[\p{White_Space}\p{Cc}]/u.test(input)) return null;
  const scheme = /^[a-z][a-z0-9+.-]*:\/\//i.exec(input);
  const relative = input.startsWith("//");
  const bare = !scheme && !relative;
  try {
    const parsed = new URL(scheme ? input : relative ? `https:${input}` : `http://${input}`);
    if (!parsed.hostname) return null;
    let rest = scheme ? input.slice(scheme[0].length) : relative ? input.slice(2) : input;
    const special = ["http:", "https:", "ftp:", "ws:", "wss:", "file:"].includes(parsed.protocol);
    if (special) rest = rest.replace(/^[\\/]+/, "");
    let authority = rest.split(special ? /[\\/?#]/ : /[/?#]/)[0];
    if (bare && authority.includes("@")) return null;
    authority = authority.slice(authority.lastIndexOf("@") + 1);
    const portText = authority.startsWith("[")
      ? authority.slice(authority.indexOf("]") + 1).replace(/^:/, "")
      : authority.includes(":")
        ? authority.slice(authority.lastIndexOf(":") + 1)
        : "";
    if (bare && !portText && authority.endsWith(":")) return null;
    if (portText && !/^\d+$/.test(portText)) return null;
    const port = portText ? Number(portText) : null;
    if (port !== null && (!Number.isInteger(port) || port > 65535)) return null;
    const host = parsed.hostname.replace(/^\[|\]$/g, "").toLowerCase();
    return { host, port };
  } catch {
    return null;
  }
}

/** Canonical destination host, without userinfo, port or IPv6 brackets. */
export function urlHost(url: string): string | null {
  return parseAuthority(url)?.host ?? null;
}

/** Return the shortest address that still matches `url` under `accuracy`, or
 *  `null` when the input is not a usable URL (the caller renders the message
 *  so user-facing text stays in the i18n dictionaries). */
export function shortestMatchable(url: string, accuracy: Accuracy): string | null {
  const trimmed = url.trim();
  const authority = parseAuthority(trimmed);
  if (!authority) return null;
  const host = authority.host.includes(":") ? `[${authority.host}]` : authority.host;
  if (accuracy === "Domain") return host;
  if (accuracy === "Hostname") return authority.port === null ? host : `${host}:${authority.port}`;
  return trimmed;
}
