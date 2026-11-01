import { createHash } from "node:crypto";

// Diagnostic custody of actual browser response bodies, never execution proof.
const maximumResponses = 4;
const maximumBytes = 20 * 1024 * 1024;
export function installWasmResponseCustody(test) {
  const states = new WeakMap();
  test.beforeEach(async ({ page }) => {
    const state = { records: [], reads: [], omitted: 0 };
    state.listener = response => {
      const headers = response.headers();
      if (!response.url().includes(".wasm") && !headers["content-type"]?.includes("application/wasm")) return;
      if (state.records.length === maximumResponses) {
        state.omitted = Math.min(65535, state.omitted + 1);
        return;
      }
      const record = { url: response.url().slice(0, 2048), status: response.status(),
        hashScope: "actual browser response body; not proof of WASM execution" };
      state.records.push(record);
      state.reads.push((async () => {
        const length = headers["content-length"];
        if (!length || !/^[0-9]{1,8}$/.test(length) || Number(length) > maximumBytes) {
          record.refusal = "missing or overbound declared response length";
          return;
        }
        const body = await response.body();
        if (body.length > maximumBytes || body.length !== Number(length)) {
          record.refusal = "response body does not match finite declared length";
          return;
        }
        record.bytes = body.length;
        record.sha256 = createHash("sha256").update(body).digest("hex");
      })().catch(error => { record.observerError = String(error).slice(0, 512); }));
    };
    states.set(page, state);
    page.on("response", state.listener);
  });
  test.afterEach(async ({ page }, info) => {
    const state = states.get(page);
    if (!state) return;
    page.off("response", state.listener);
    states.delete(page);
    await Promise.all(state.reads);
    await info.attach("wasm-response-custody", { contentType: "application/json",
      body: JSON.stringify({ maximumResponses, maximumBytes, omitted: state.omitted, records: state.records }) });
  });
}
