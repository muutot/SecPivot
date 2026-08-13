import { spawnSync } from "node:child_process";

export function isReleaseCommitSubject(subject, version) {
  return subject === `🔖 chore[release]: bump version to ${version}`;
}

export function isGitAncestor(cwd, ancestor, descendant = "HEAD") {
  const result = spawnSync("git", ["merge-base", "--is-ancestor", ancestor, descendant], {
    cwd,
    encoding: "utf-8",
    stdio: "pipe",
  });
  if (result.status === 0) return true;
  if (result.status === 1) return false;
  throw new Error(
    `Unable to verify release ancestry: ${(result.stderr || result.error?.message || "git failed").trim()}`,
  );
}
