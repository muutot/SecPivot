/** Compute a "shortest matchable address" for a KeePassRPC per-entry rule,
 * mirroring the backend's 3-tier match (`Exact`/`Hostname`/`Domain`).
 *
 * Given the real URL Kee reported (e.g. `FindLogins urls=[...]`), return the
 * shortest string that still matches under the currently selected accuracy:
 * - `Exact`: the full URL with any query/fragment stripped (path preserved).
 * - `Hostname`: `host[:port]` only (path dropped).
 * - `Domain`: the host, kept scoped so only the host and its subdomains match
 *   (e.g. `passport.aliyun.com` covers `passport.aliyun.com` and deeper hosts,
 *   but never spreads to the bare registrable domain `aliyun.com`).
 */

type Accuracy = "Exact" | "Hostname" | "Domain";

/** Lower-cased host portion of a URL (`host[:port]`), or `null` when unparsable.
 */
export function urlHost(url: string): string | null {
  const m = /^[a-z][a-z0-9+.-]*:\/\//i.exec(url);
  const rest = (m ? url.slice(m[0].length) : url).split(/[/?#]/)[0];
  const host = rest.replace(/^\[/, "").replace(/\]$/, "").split(":")[0].toLowerCase();
  return host || null;
}

/** Strip `query` and `fragment` from a URL, keeping scheme + host + path.
 */
function withoutQueryFragment(url: string): string {
  const q = url.search(/[?#]/);
  return q < 0 ? url : url.slice(0, q);
}

/** Return the shortest address that still matches `url` under `accuracy`, or a
 * human message when the input is not a usable URL. */
export function shortestMatchable(url: string, accuracy: Accuracy): string {
  const trimmed = url.trim();
  const host = urlHost(trimmed);
  if (!host) return "无法识别:请输入完整网址(含 https://)";
  if (accuracy === "Domain" || accuracy === "Hostname") return host;
  const withScheme = /^https?:\/\//i.test(trimmed) ? trimmed : `https://${trimmed}`;
  return withoutQueryFragment(withScheme);
}
