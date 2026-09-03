const VERSION = 2;
const RETIRED_VERSION = 1;

const COLOR_ROLES = Object.freeze([
  "background", "reading-paper", "workbench-canvas", "bootstrap-surface", "surface",
  "structure-primary", "structure-secondary", "text-primary", "text-secondary", "emphasis",
  "focus", "warning", "failure", "success", "muted",
]);
const METRIC_ROLES = Object.freeze([
  ["type-body", "px"], ["type-small", "px"], ["line-height", "%"], ["space-unit", "px"],
  ["space-control-inline", "px"], ["space-control-block", "px"], ["space-panel", "px"],
  ["radius-control", "px"], ["radius-panel", "px"], ["focus-width", "px"],
  ["responsive-breakpoint", "px"], ["responsive-grid-min", "px"],
]);

export function decodeTheme(input, refuse) {
  const encoded = input instanceof Uint8Array ? input : new Uint8Array(input);
  let offset = 0;
  const bytes = (length) => {
    const end = offset + length;
    if (!Number.isSafeInteger(end) || end > encoded.length) refuse("malformed-encoding");
    const value = encoded.slice(offset, end);
    offset = end;
    return value;
  };
  if (bytes(1)[0] !== VERSION) refuse("unsupported-version");
  const identityLength = bytes(1)[0];
  if (identityLength === 0 || identityLength > 64) refuse("text-too-long");
  let identity;
  try { identity = new TextDecoder("utf-8", { fatal: true }).decode(bytes(identityLength)); }
  catch { refuse("malformed-encoding"); }
  const tokens = {};
  for (const role of COLOR_ROLES) {
    const [red, green, blue] = bytes(3);
    tokens[role] = `#${[red, green, blue].map((value) => value.toString(16).padStart(2, "0")).join("")}`;
  }
  for (const [role, unit] of METRIC_ROLES) {
    const metric = bytes(2);
    tokens[role] = `${metric[0] | (metric[1] << 8)}${unit}`;
  }
  if (offset !== encoded.length) refuse("malformed-encoding");
  return Object.freeze({ identity, tokens: Object.freeze(tokens) });
}

export const applicationThemeLimits = Object.freeze({ version: VERSION, retiredVersion: RETIRED_VERSION });
