import { openBrowserHumanInput } from "../../targets/browser/host/assets/browser-human-input.mjs";

const REQUIRED_EXPORTS = [
  "memory",
  "conduit_browser_pointer_run",
  "conduit_browser_pointer_receipt_ptr",
  "conduit_browser_pointer_receipt_len",
];

export async function installBrowserPointerSource(wasmBytes, target, onReceipt) {
  if (!(target instanceof Element)) throw new TypeError("pointer source requires an Element");
  const { instance } = await WebAssembly.instantiate(wasmBytes, {});
  const api = instance.exports;
  for (const name of REQUIRED_EXPORTS) {
    if (!(name in api)) throw new Error(`missing browser pointer export: ${name}`);
  }
  let closed = false;
  const boot = Object.freeze({
    host_id: "browser-pointer-host",
    boot_id: "browser-pointer-boot",
    offer_generation: 1,
    implementation_registry: Object.freeze([{ id: "browser/pointer-events@1", revision: 1 }]),
  });
  const adapter = openBrowserHumanInput({ target, boot });
  const stop = adapter.observePointer((event, error) => {
    if (error) throw error;
    if (closed || !event) return;
    const status = api.conduit_browser_pointer_run(
      event.position_x,
      event.position_y,
      event.delta_x,
      event.delta_y,
      event.primary_pressed ? 1 : 0,
      event.coalesced,
      event.dropped,
      event.queue_capacity,
      event.sequence,
    );
    if (status !== 0) throw new Error(`browser pointer Play refused ${status}`);
    const pointer = api.conduit_browser_pointer_receipt_ptr();
    const length = api.conduit_browser_pointer_receipt_len();
    if (pointer === 0 || length === 0 || length > 2_048) {
      throw new Error("browser pointer receipt is outside its admitted bound");
    }
    const receipt = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(
      new Uint8Array(api.memory.buffer, pointer, length),
    ));
    onReceipt(Object.freeze(receipt));
  });
  return Object.freeze({
    api,
    adapter,
    close() {
      if (!closed) stop();
      adapter.close();
      closed = true;
    },
  });
}
