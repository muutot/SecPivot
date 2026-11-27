import type { BitwardenUriMatch, BitwardenUriSettings } from "$lib/types/vault";

/** Source-account choices only; never saved as application preferences. */
export function copyBitwardenUriSettings(value: BitwardenUriSettings): BitwardenUriSettings {
  if (
    !value ||
    !Number.isInteger(value.defaultMatch) ||
    value.defaultMatch < 0 ||
    value.defaultMatch > 5 ||
    !Array.isArray(value.equivalentDomains) ||
    value.equivalentDomains.length > 128
  )
    throw new Error("Invalid Bitwarden URI settings");
  let bytes = 0;
  const equivalentDomains = value.equivalentDomains.map((group) => {
    if (!Array.isArray(group) || group.length > 128)
      throw new Error("Invalid Bitwarden URI settings");
    return group.map((domain) => {
      if (typeof domain !== "string") throw new Error("Invalid Bitwarden URI settings");
      const size = new TextEncoder().encode(domain).length;
      bytes += size;
      if (!domain || size > 1024 || bytes > 32768 || /[\s\u0000-\u001f\u007f-\u009f]/u.test(domain))
        throw new Error("Invalid Bitwarden URI settings");
      return domain;
    });
  });
  return { defaultMatch: value.defaultMatch, equivalentDomains };
}

export function parseBitwardenUriSettings(
  defaultMatch: BitwardenUriMatch,
  text: string,
): BitwardenUriSettings {
  return copyBitwardenUriSettings({
    defaultMatch,
    equivalentDomains: text
      .split(/\r?\n/)
      .map((row) =>
        row
          .split(/[,，]/)
          .map((domain) => domain.trim())
          .filter(Boolean),
      )
      .filter((group) => group.length),
  });
}
