import { readFile } from "node:fs/promises";
import { join } from "node:path";

const artifactDir = process.argv[2];
if (!artifactDir) {
  throw new Error("WASM bridge check requires the generated artifact directory");
}

const declarationsPath = join(artifactDir, "conduit_web.d.ts");
const declarations = await readFile(declarationsPath, "utf8");
const requiredOperations = [
  "panel_source_metadata",
  "patchbay_open_session",
  "patchbay_session_view",
  "patchbay_apply_transaction",
  "patchbay_start_exact_run",
  "patchbay_pump_exact_run",
  "patchbay_read_exact_evidence",
  "patchbay_attach_exact_watch",
  "patchbay_detach_exact_watch",
  "patchbay_read_exact_watch",
  "patchbay_advance_exact_run",
  "patchbay_notify_host_operation",
  "patchbay_cancel_exact_run",
  "patchbay_snapshot_exact_run",
  "patchbay_dispose_exact_run",
];

const missing = requiredOperations.filter(
  (operation) => !declarations.includes(`function ${operation}(`),
);
if (missing.length > 0) {
  throw new Error(
    `Generated WASM bridge omits required operations:\n${missing.join("\n")}`,
  );
}

console.log("Generated WASM bridge exports every required session operation.");
