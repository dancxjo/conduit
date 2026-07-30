import init, {
  run_panel,
  run_panel_exact,
} from "./conduit_web.js";
import {
  BrowserHostReason,
  verifyExactArtifact,
} from "../../browser/conduit-browser-host.mjs";

let configured = false;

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
    if (operation === "run" && configured) {
      globalThis.postMessage({
        id,
        ok: true,
        value: JSON.parse(run_panel(value.source)),
      });
      return;
    }
    if (operation === "run-exact" && configured) {
      globalThis.postMessage({
        id,
        ok: true,
        value: JSON.parse(run_panel_exact(value.source, value.compileInputJson)),
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
