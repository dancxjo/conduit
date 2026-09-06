const mode = process.argv[2];

function requireExact(actual, expected, name) {
  if (actual !== expected) {
    throw new Error(`${name} must be ${expected}; received ${actual || "<empty>"}`);
  }
}

function requireCommitSha(actual, name) {
  if (!/^[0-9a-f]{40}$/.test(actual || "")) {
    throw new Error(`${name} must be a full lowercase Git commit SHA; received ${actual || "<empty>"}`);
  }
}

export function validateBoundary(mode, environment) {
  if (mode === "candidate") {
    requireExact(environment.CONDUIT_EVENT_NAME, "pull_request", "event");
    requireExact(environment.CONDUIT_BASE_REF, "dev", "candidate base");
    return { mode, admission: "focused", target: "dev" };
  }
  if (mode === "dev") {
    requireExact(environment.CONDUIT_EVENT_NAME, "push", "event");
    requireExact(environment.CONDUIT_REF_NAME, "dev", "integration ref");
    return { mode, admission: "integration-smoke", target: "dev" };
  }
  if (mode === "promotion") {
    requireExact(environment.CONDUIT_EVENT_NAME, "pull_request", "event");
    requireExact(environment.CONDUIT_BASE_REF, "main", "promotion base");
    requireExact(environment.CONDUIT_HEAD_REPOSITORY, environment.CONDUIT_REPOSITORY, "promotion repository");
    requireCommitSha(environment.CONDUIT_HEAD_SHA, "promotion snapshot identity");
    const expectedRef = `promote/${environment.CONDUIT_HEAD_SHA}`;
    requireExact(environment.CONDUIT_HEAD_REF, expectedRef, "promotion snapshot ref");
    return {
      mode,
      admission: "exhaustive",
      source: "frozen-dev-snapshot",
      snapshot: environment.CONDUIT_HEAD_SHA,
      target: "main",
    };
  }
  throw new Error(`unknown branch boundary ${mode || "<empty>"}`);
}

if (import.meta.url === `file://${process.argv[1]}`) {
  try {
    process.stdout.write(`${JSON.stringify(validateBoundary(mode, process.env))}\n`);
  } catch (error) {
    process.stderr.write(`${error.message}\n`);
    process.exitCode = 1;
  }
}
