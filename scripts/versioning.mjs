const BUMP_TYPES = new Set(["major", "minor", "patch"]);

export function parseSemver(version) {
  const match = version.match(
    /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?$/,
  );
  if (!match) throw new Error(`Invalid semver: ${version}`);
  const prerelease = match[4] || "";
  if (
    prerelease.split(".").some((identifier) => /^\d+$/.test(identifier) && /^0\d/.test(identifier))
  ) {
    throw new Error(`Invalid semver: ${version}`);
  }
  return {
    major: Number.parseInt(match[1], 10),
    minor: Number.parseInt(match[2], 10),
    patch: Number.parseInt(match[3], 10),
    pre: prerelease ? `-${prerelease}` : "",
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
    const explicitVersion = target.replace(/^v/, "");
    return formatSemver(parseSemver(explicitVersion));
  }
  return formatSemver(version);
}

export function isBumpType(value) {
  return BUMP_TYPES.has(value);
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
