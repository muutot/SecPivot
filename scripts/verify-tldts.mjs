import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
const files = {
  "src-tauri/vendor/tldts-7.4.2/index.cjs.min.js":
    "fd2168b91e566b90616774678e73e7a91010ce20da3d35817b5ed8a2abc93856",
  "src-tauri/vendor/tldts-7.4.2/LICENSE":
    "c64182d48160db948b6aa97a984f1a32974ce4d2053900361264cd33853796cb",
};
for (const [path, expected] of Object.entries(files)) {
  const bytes = process.argv.includes("--staged")
    ? execFileSync("git", ["show", `:${path}`], { maxBuffer: 1024 * 1024 })
    : readFileSync(new URL(`../${path}`, import.meta.url));
  if (createHash("sha256").update(bytes).digest("hex") !== expected)
    throw new Error(`Unexpected tldts vendor bytes: ${path}`);
}
console.log("Verified unchanged tldts 7.4.2 bundle and MIT license");
