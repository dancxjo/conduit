const { instance } = await WebAssembly.instantiate(
  await (await fetch("../../target/wasm32-unknown-unknown/release/conduit_browser_runtime.wasm")).arrayBuffer(), {},
);
const api = instance.exports;
const authored = await (await fetch("../../forms/quantity-range-map/main.conduit")).text();
const source = `${authored}
form zz-pointer-quantity {
 pointer: input/pointer-source
 normalize: math/normalized-quantity-scalar
 map: quantity-range-map
 wrap: structured-info/wrap-quantity
 show: presentation/structured-info
 pointer.pointer > project(PointerEvent.position) > project(Point2.x) > normalize.in
 normalize.out > map.control
 map.quantity > wrap.in
 wrap.out > show.input
}`;
const encoder = new TextEncoder();
const decoder = new TextDecoder();
const sourceBytes = encoder.encode(source);
const host = encoder.encode("browser/quantity-controller");
const boot = encoder.encode("browser/quantity-controller-boot");
let sequence = 0;
const readBytes = () => new Uint8Array(api.memory.buffer,
  api.conduit_browser_form_output_ptr(), api.conduit_browser_form_output_len()).slice();
const read = () => JSON.parse(decoder.decode(readBytes()));
const write = (bytes) => {
  if (bytes.length > api.conduit_browser_form_input_capacity()) throw new Error("input bound exceeded");
  new Uint8Array(api.memory.buffer, api.conduit_browser_form_input_ptr(), bytes.length).set(bytes);
};
const accept = (code, stage) => { if (code < 0) throw new Error(`${stage} refused (${code})`); };

function execute(x, y, inputMode) {
  try {
    sequence += 1;
    write(sourceBytes);
    accept(api.conduit_browser_form_admit_source_interaction(sourceBytes.length, BigInt(sequence)), "source");
    const input = new Uint8Array(host.length + boot.length + sourceBytes.length);
    input.set(host); input.set(boot, host.length); input.set(sourceBytes, host.length + boot.length);
    write(input);
    accept(api.conduit_browser_form_start_quantity(host.length, boot.length, sourceBytes.length, BigInt(sequence)), "start");
    const acquisition = read();
    if (acquisition.effect_kind !== "pointer-event") throw new Error("expected pointer effect");
    accept(api.conduit_browser_form_encode_pointer(x, y, 0, 0, 0, 0, 0, 1, sequence), "pointer codec");
    const canonical = readBytes();
    write(canonical);
    accept(api.conduit_browser_form_complete_with_output(canonical.length), "pointer completion");
    const effect = read();
    if (effect.effect_kind !== "manifestation") throw new Error("expected manifestation");
    document.querySelector("#output").textContent = effect.text;
    accept(api.conduit_browser_form_complete(), "presentation completion");
    const receipt = read();
    if (receipt.active_play_id !== effect.active_play_id || acquisition.active_play_id !== effect.active_play_id)
      throw new Error("Play correlation drift");
    document.querySelector("#evidence").textContent = JSON.stringify({ inputMode, x, acquisition, effect, receipt });
    document.querySelector("#status").textContent = `Completed ${sequence}`;
  } catch (error) {
    document.querySelector("#status").textContent = String(error);
    throw error;
  }
}

document.querySelector("#pointer").addEventListener("pointerdown", (event) => {
  const box = event.currentTarget.getBoundingClientRect();
  // Acquisition expresses normalized pointer coordinates only; Conduit owns mapping.
  execute(Math.round((event.clientX - box.left) * 1_000_000 / box.width),
    Math.round((event.clientY - box.top) * 1_000_000 / box.height), "pointer");
});
document.querySelector("#alternate").addEventListener("click", () => execute(750_000, 0, "deterministic"));
document.querySelector("#status").textContent = "Ready";
