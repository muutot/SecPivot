const BUMP_TYPES = new Set(["major", "minor", "patch"]);

export function parseSemver(version) {
  const match = version.match(/^(\d+)\.(\d+)\.(\d+)(-.+)?$/);
  if (!match) throw new Error(`Invalid semver: ${version}`);
  return {
    major: Number.parseInt(match[1], 10),
    minor: Number.parseInt(match[2], 10),
    patch: Number.parseInt(match[3], 10),
    pre: match[4] || "",
  };
}

function formatSemver({ major, minor, patch, pre }) {
  return `${major}.${minor}.${patch}${pre}`;
}

export function bumpVersion(current, target) {
  const version = parseSemver(current);
  if (target === "major") {
    version.major += 1;
    version.minor = 0;
    version.patch = 0;
    version.pre = "";
  } else if (target === "minor") {
    version.minor += 1;
    version.patch = 0;
    version.pre = "";
  } else if (target === "patch") {
    version.patch += 1;
    version.pre = "";
  } else {
    return target.replace(/^v/, "");
  }
  return formatSemver(version);
}

export function resolveReleaseTarget(currentVersion, committedVersion, requestedVersion) {
  if (!BUMP_TYPES.has(requestedVersion)) {
    return bumpVersion(currentVersion, requestedVersion);
  }

  const targetVersion = bumpVersion(committedVersion, requestedVersion);
  if (currentVersion === committedVersion || currentVersion === targetVersion) {
    return targetVersion;
  }

  throw new Error(
    `Cannot apply ${requestedVersion}: working version ${currentVersion} is neither HEAD ${committedVersion} nor expected target ${targetVersion}.`,
  );
}
