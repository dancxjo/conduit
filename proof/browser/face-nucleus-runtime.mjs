import { manifestApplicationView } from "../../targets/browser/host/assets/application-face.mjs";

const REQUIRED_EXPORTS = [
  "memory",
  "conduit_browser_face_nucleus_run",
  "conduit_browser_face_nucleus_application_view_ptr",
  "conduit_browser_face_nucleus_application_view_len",
  "conduit_browser_face_nucleus_look_ptr",
  "conduit_browser_face_nucleus_look_len",
  "conduit_browser_face_nucleus_graphics_ptr",
  "conduit_browser_face_nucleus_graphics_len",
  "conduit_browser_face_nucleus_layout_ptr",
  "conduit_browser_face_nucleus_layout_len",
  "conduit_browser_face_nucleus_text_ptr",
  "conduit_browser_face_nucleus_text_len",
  "conduit_browser_face_nucleus_structured_ptr",
  "conduit_browser_face_nucleus_structured_len",
];

const rect = (view, offset) => ({
  x: view.getInt16(offset, true),
  y: view.getInt16(offset + 2, true),
  width: view.getUint16(offset + 4, true),
  height: view.getUint16(offset + 6, true),
});

function bytes(api, pointerExport, lengthExport) {
  const pointer = api[pointerExport]();
  const length = api[lengthExport]();
  if (pointer === 0 || length === 0) throw new Error("empty browser nucleus output");
  return new Uint8Array(api.memory.buffer, pointer, length).slice();
}

function decodeLayout(encoded) {
  if (encoded.length < 10 || encoded[0] !== 1) throw new Error("invalid layout frame");
  const count = encoded[1];
  if (encoded.length !== 10 + count * 8) throw new Error("non-canonical layout frame");
  const view = new DataView(encoded.buffer, encoded.byteOffset, encoded.byteLength);
  return {
    viewport: rect(view, 2),
    children: Array.from({ length: count }, (_, index) => rect(view, 10 + index * 8)),
  };
}

function decodeGraphics(encoded) {
  if (encoded.length < 2 || encoded[0] !== 2) throw new Error("invalid graphics scene");
  const count = encoded[1];
  const view = new DataView(encoded.buffer, encoded.byteOffset, encoded.byteLength);
  const decoder = new TextDecoder();
  const commands = [];
  let offset = 2;
  for (let index = 0; index < count; index += 1) {
    if (encoded.length - offset < 21) throw new Error("truncated graphics command");
    const payloadLength = encoded[offset + 19];
    const textRole = encoded[offset + 20];
    if (textRole < 1 || textRole > 9 || (encoded[offset] !== 2 && textRole !== 1)) {
      throw new Error("invalid graphics text role");
    }
    const end = offset + 21 + payloadLength;
    if (end > encoded.length) throw new Error("truncated graphics payload");
    commands.push({
      kind: encoded[offset],
      paint: encoded[offset + 1],
      style: encoded[offset + 2],
      textRole: [undefined, "body", "label", "heading", "title", "code", "status", "warning", "action", "muted"][textRole],
      bounds: rect(view, offset + 3),
      clip: rect(view, offset + 11),
      payload: decoder.decode(encoded.slice(offset + 21, end)),
    });
    offset = end;
  }
  if (offset !== encoded.length) throw new Error("non-canonical graphics scene");
  return commands;
}

function decodeStructured(encoded) {
  if (encoded.length < 5 || encoded[0] !== 3) throw new Error("invalid structured face frame");
  const decoder = new TextDecoder("utf-8", { fatal: true });
  const fields = [];
  let offset = 1;
  for (let index = 0; index < 5; index += 1) {
    if (offset >= encoded.length) throw new Error("truncated structured face frame");
    const length = encoded[offset];
    offset += 1;
    const end = offset + length;
    if (end > encoded.length) throw new Error("truncated structured face frame");
    fields.push(decoder.decode(encoded.slice(offset, end)));
    offset = end;
  }
  if (offset !== encoded.length) throw new Error("non-canonical structured face frame");
  return Object.freeze({ schema: fields[0], variant: fields[1], quantityUnit: fields[2], quantity: fields[3], quantityUnitSymbol: fields[4] });
}

export async function instantiateFaceNucleus(wasmBytes) {
  const { instance } = await WebAssembly.instantiate(wasmBytes, {});
  for (const name of REQUIRED_EXPORTS) {
    if (!(name in instance.exports)) throw new Error(`missing browser nucleus export: ${name}`);
  }
  return instance.exports;
}

export function manifestFaceNucleus(api, root) {
  if (!(root instanceof Element)) throw new TypeError("browser nucleus requires an Element");
  if (api.conduit_browser_face_nucleus_run() !== 0) {
    throw new Error("browser nucleus kernel execution failed");
  }
  const layout = decodeLayout(bytes(
    api,
    "conduit_browser_face_nucleus_layout_ptr",
    "conduit_browser_face_nucleus_layout_len",
  ));
  const graphics = decodeGraphics(bytes(
    api,
    "conduit_browser_face_nucleus_graphics_ptr",
    "conduit_browser_face_nucleus_graphics_len",
  ));
  const text = new TextDecoder().decode(bytes(
    api,
    "conduit_browser_face_nucleus_text_ptr",
    "conduit_browser_face_nucleus_text_len",
  ));
  const structured = decodeStructured(bytes(
    api,
    "conduit_browser_face_nucleus_structured_ptr",
    "conduit_browser_face_nucleus_structured_len",
  ));
  const applicationLook = bytes(
    api,
    "conduit_browser_face_nucleus_look_ptr",
    "conduit_browser_face_nucleus_look_len",
  );
  const application = manifestApplicationView(bytes(
    api,
    "conduit_browser_face_nucleus_application_view_ptr",
    "conduit_browser_face_nucleus_application_view_len",
  ), root, { look: applicationLook });

  const applicationShell = root.firstElementChild;
  root.dataset.viewport = `${layout.viewport.width}x${layout.viewport.height}`;
  for (const [index, placement] of layout.children.entries()) {
    const child = document.createElement("section");
    child.dataset.layoutIndex = String(index);
    child.dataset.layoutRect = `${placement.x},${placement.y},${placement.width},${placement.height}`;
    applicationShell.append(child);
  }
  for (const [index, command] of graphics.entries()) {
    const leaf = document.createElement(command.kind === 2 ? "span" : "div");
    leaf.dataset.graphicsIndex = String(index);
    leaf.dataset.graphicsKind = ["", "rect", "text", "icon"][command.kind] ?? "unknown";
    if (command.kind === 2) leaf.dataset.graphicsTextRole = command.textRole;
    leaf.dataset.clip = `${command.clip.x},${command.clip.y},${command.clip.width},${command.clip.height}`;
    leaf.textContent = command.payload;
    if (command.kind === 3) leaf.setAttribute("role", "img");
    applicationShell.append(leaf);
  }
  const presentedText = document.createElement("output");
  presentedText.dataset.faceKind = "text";
  presentedText.textContent = text;
  applicationShell.append(presentedText);
  const structuredFace = document.createElement("output");
  structuredFace.dataset.faceKind = "structured-info";
  structuredFace.dataset.schema = structured.schema;
  structuredFace.dataset.variant = structured.variant;
  structuredFace.dataset.quantityUnit = structured.quantityUnit;
  structuredFace.dataset.quantityUnitSymbol = structured.quantityUnitSymbol;
  structuredFace.dataset.quantity = String(structured.quantity);
  structuredFace.setAttribute("aria-label", "Education feedback structured information");
  applicationShell.append(structuredFace);
  return Object.freeze({ layout, graphics, text, structured, application, applicationLook });
}
