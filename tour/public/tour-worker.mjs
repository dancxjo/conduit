import init, {
  cancel_panel,
  patchbay_advance_exact_run,
  patchbay_cancel_exact_run,
  patchbay_notify_host_operation,
  patchbay_open_session,
  patchbay_pump_exact_run,
  patchbay_session_view,
  patchbay_start_exact_run,
  run_panel,
  run_panel_exact,
} from "./conduit_web.js";
import {
  BrowserHostReason,
  verifyExactArtifact,
} from "../../browser/conduit-browser-host.mjs";

let configured = false;

function response(operation, value) {
  switch (operation) {
    case "run":
      return run_panel(value.source);
    case "cancel":
      return cancel_panel(value.source);
    case "run-exact":
      return run_panel_exact(value.source, value.compileInputJson);
    case "patchbay-open-session":
      return patchbay_open_session(value.documentId, value.source);
    case "patchbay-session-view":
      return patchbay_session_view(value.sessionId);
    case "patchbay-start-exact-run":
      return patchbay_start_exact_run(value.sessionId);
    case "patchbay-pump-exact-run":
      return patchbay_pump_exact_run(value.sessionId, value.quantum);
    case "patchbay-advance-exact-run":
      return patchbay_advance_exact_run(value.sessionId, value.tick);
    case "patchbay-notify-host-operation":
      return patchbay_notify_host_operation(value.sessionId, value.subject);
    case "patchbay-cancel-exact-run":
      return patchbay_cancel_exact_run(value.sessionId, value.disposition);
    default:
      return undefined;
  }
}

async function configure(value) {
  if (configured) return { configured: true };
  const response = await fetch(value.wasmUrl, { cache: "no-store" });
  if (!response.ok) {
    throw new Error(`artifact-fetch:${response.status}`);
  }
  const bytes = await response.arrayBuffer();
  const verified = await verifyExactArtifact(bytes, value.wasmSha256);
  if (!verified.ok) {
    throw new Error(`${BrowserHostReason.ArtifactIntegrity}:${verified.detail}`);
  }
  await init({ module_or_path: bytes });
  configured = true;
  return { configured: true };
}

globalThis.onmessage = async (event) => {
  const { id, operation, value } = event.data ?? {};
  try {
    if (operation === "configure") {
      globalThis.postMessage({ id, ok: true, value: await configure(value) });
      return;
    }
    const result = configured && response(operation, value);
    if (result !== undefined) {
      globalThis.postMessage({
        id,
        ok: true,
        value: JSON.parse(result),
      });
      return;
    }
    globalThis.postMessage({ id, ok: false, code: "unsupported-operation" });
  } catch (error) {
    globalThis.postMessage({
      id,
      ok: false,
      code: "tour-worker-failed",
      value: { diagnostic: String(error) },
    });
  }
};
