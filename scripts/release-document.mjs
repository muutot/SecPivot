export function hasReleaseHeading(content, version) {
  const normalized = content.replace(/^\uFEFF/, "").replace(/\r\n/g, "\n");
  return normalized.split("\n", 1)[0] === `# SecPivot Desktop v${version}`;
}
