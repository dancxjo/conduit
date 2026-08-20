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
  let sequence = 0;
  let closed = false;
  const listener = (event) => {
    if (closed) return;
    if (event.buttons !== 0 && event.buttons !== 1) {
      throw new Error("browser pointer buttons exceed the admitted primary-button profile");
    }
    const bounds = target.getBoundingClientRect();
    if (bounds.width <= 0 || bounds.height <= 0) throw new Error("pointer surface has no extent");
    const millionth = (value) => Math.round(value * 1_000_000);
    const positionX = millionth((event.clientX - bounds.left) / bounds.width);
    const positionY = millionth((event.clientY - bounds.top) / bounds.height);
    const deltaX = millionth(event.movementX / bounds.width);
    const deltaY = millionth(event.movementY / bounds.height);
    const coalesced = typeof event.getCoalescedEvents === "function"
      ? Math.max(0, event.getCoalescedEvents().length - 1)
      : 0;
    const status = api.conduit_browser_pointer_run(
      positionX,
      positionY,
      deltaX,
      deltaY,
      event.buttons === 1 ? 1 : 0,
      coalesced,
      0,
      1,
      sequence,
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
    sequence += 1;
    onReceipt(Object.freeze(receipt));
  };
  target.addEventListener("pointerdown", listener);
  return Object.freeze({
    api,
    close() {
      if (!closed) target.removeEventListener("pointerdown", listener);
      closed = true;
    },
  });
}
