export const RELEASE_FILES = Object.freeze([
  "package.json",
  "src-tauri/tauri.conf.json",
  "src-tauri/Cargo.toml",
  "src-tauri/Cargo.lock",
  "CHANGELOG.md",
  "RELEASE.md",
]);

const RELEASE_FILE_SET = new Set(RELEASE_FILES);

export function findUnexpectedReleaseChanges(...fileLists) {
  return [...new Set(fileLists.flat().filter(Boolean))]
    .filter((file) => !RELEASE_FILE_SET.has(file))
    .sort();
}
