const mode = process.argv[2];

function requireExact(actual, expected, name) {
  if (actual !== expected) {
    throw new Error(`${name} must be ${expected}; received ${actual || "<empty>"}`);
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
    requireExact(environment.CONDUIT_HEAD_REF, "dev", "promotion source");
    requireExact(environment.CONDUIT_HEAD_REPOSITORY, environment.CONDUIT_REPOSITORY, "promotion repository");
    return { mode, admission: "exhaustive", source: "dev", target: "main" };
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
