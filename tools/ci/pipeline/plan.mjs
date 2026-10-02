// Selection is deliberately conservative: an unrecognized path runs every family.
export const FAMILIES = Object.freeze([
  "browser", "hosted", "conduitos", "esp32", "avr", "raspberry-pi", "orange-pi", "rp2040",
]);

const rootProse = new Set([
  "README.md", "STATUS.md", "CONTRIBUTING.md", "AGENTS.md", "LICENSE",
  "LICENSE.md", "CHANGELOG.md", "CODE_OF_CONDUCT.md", "SECURITY.md",
]);

function isDocumentation(path) {
  return rootProse.has(path) || /^docs\/.+\.md$/.test(path);
}

function familiesFor(path) {
  // Dependency and toolchain edits can affect consumers outside their directory.
  if (/(^|\/)(Cargo\.(toml|lock)|package(-lock)?\.json|rust-toolchain(\.toml)?)$/.test(path)) {
    return FAMILIES;
  }
  // These target subtrees are libraries consumed across target families.
  // Keep this conservative rather than reconstructing Cargo's dependency graph.
  if (/^targets\/[^/]+\/(runtime|offers|make|network-realization)\//.test(path)) return FAMILIES;
  if (/^(targets\/browser|proof\/browser|site)\//.test(path)) return ["browser"];
  // The browser lane executes hosted admission and WebChat helper binaries.
  if (path.startsWith("targets/std/")) return ["browser", "hosted"];
  // Orange Pi media uses the shared ConduitOS implementation.
  if (path.startsWith("targets/conduitos/")) return ["conduitos", "orange-pi"];
  for (const family of ["esp32", "avr", "raspberry-pi", "orange-pi", "rp2040"]) {
    if (path.startsWith(`targets/${family}/`)) return [family];
  }
  return FAMILIES;
}

/**
 * Paths are repository-relative names, including both sides of renames.
 * Empty or unknown input fails closed. Integration passes full: true.
 */
export function planChanges(paths, { full = false } = {}) {
  if (!Array.isArray(paths) || paths.some((path) => typeof path !== "string")) {
    throw new TypeError("changed paths must be an array of strings");
  }
  if (full || paths.length === 0) return { docsOnly: false, families: [...FAMILIES] };

  const selected = new Set();
  for (const path of paths) {
    if (isDocumentation(path)) continue;
    for (const family of familiesFor(path)) selected.add(family);
  }
  return {
    docsOnly: selected.size === 0,
    families: FAMILIES.filter((family) => selected.has(family)),
  };
}
