// Proof reader for the shared Rust graphics-scene@2 contract. Not an owner,
// planner, or production rendering authority. Binary geometry stays binary.
const roles = [undefined, "body", "title", "heading", "label", "code", "status", "warning", "action", "muted", "value"];
const icons = new Set([
  "clock", "repeat-2", "face", "type", "case-upper", "combine", "tally-5",
  "chart-no-axes-column-increasing", "file-output", "keyboard", "conduit-generic-gear",
  "conduit-body", "conduit-wake", "conduit-plan", "conduit-play", "conduit-host",
  "conduit-port", "conduit-line", "status", "warning", "close", "conduit-back",
  "conduit-confirm", "conduit-search", "conduit-inspect", "conduit-lull",
  "conduit-library", "conduit-collection", "conduit-add", "conduit-remove",
]);
const refuse = () => { throw new Error("non-canonical graphics scene"); };
function rectangle(view, offset) {
  const value = { x: view.getInt16(offset, true), y: view.getInt16(offset + 2, true), width: view.getUint16(offset + 4, true), height: view.getUint16(offset + 6, true) };
  if (!value.width || !value.height || value.width > 32767 || value.height > 32767 || value.x + value.width > 32767 || value.y + value.height > 32767) refuse();
  return value;
}
function points(payload, maximum, bounds, orthogonal) {
  if (payload.length < 8 || payload.length > maximum * 4 || payload.length % 4) refuse();
  const view = new DataView(payload.buffer, payload.byteOffset, payload.byteLength);
  const result = Array.from({ length: payload.length / 4 }, (_, index) => ({x:view.getInt16(index * 4, true), y:view.getInt16(index * 4 + 2, true)}));
  for (let index = 1; index < result.length; index++) {
    const a = result[index - 1], b = result[index];
    if ((a.x === b.x && a.y === b.y) || (orthogonal && a.x !== b.x && a.y !== b.y)) refuse();
  }
  const xs = result.map(p => p.x), ys = result.map(p => p.y);
  if (bounds.x !== Math.min(...xs) || bounds.y !== Math.min(...ys) || bounds.width !== Math.max(...xs) - bounds.x + 1 || bounds.height !== Math.max(...ys) - bounds.y + 1) refuse();
  return result;
}
export function decodeGraphics(encoded) {
  if (!(encoded instanceof Uint8Array) || encoded.length < 2 || encoded.length > 10226 || ![2, 3].includes(encoded[0]) || encoded[1] > 48) refuse();
  const view = new DataView(encoded.buffer, encoded.byteOffset, encoded.byteLength);
  const decoder = new TextDecoder("utf-8", {fatal:true});
  const commands = [];
  let offset = 2, hasMetrics = false;
  for (let index = 0; index < encoded[1]; index++) {
    if (encoded.length - offset < 21) refuse();
    const kind = encoded[offset], paint = encoded[offset + 1], style = encoded[offset + 2];
    const length = encoded[offset + 19], role = encoded[offset + 20], end = offset + 21 + length;
    if (kind < 1 || kind > 5 || paint < 1 || paint > 13 || style < 1 || style > 4 || length > 192 || !roles[role] || (kind !== 2 && role !== 1) || end > encoded.length) refuse();
    const bounds = rectangle(view, offset + 3), clip = rectangle(view, offset + 11);
    const raw = encoded.slice(offset + 21, end);
    const command = {kind,paint,style,textRole:roles[role],bounds,clip,payload:""};
    if (kind === 1) {
      if (length) {
        if (length !== 2 || raw[0] > 32 || raw[1] < 1 || raw[1] > 8 || style < 3) refuse();
        command.rectMetrics = {radius:raw[0],stroke:raw[1]}; hasMetrics = true;
      }
    } else if (kind === 4 || kind === 5) {
      if (style !== 2) refuse();
      command.points = points(raw, kind === 4 ? 8 : 16, bounds, kind === 4);
    } else {
      if (!length || style !== 1) refuse();
      command.payload = decoder.decode(raw);
      if (kind === 3 && !icons.has(command.payload)) refuse();
    }
    commands.push(command); offset = end;
  }
  if (offset !== encoded.length || (encoded[0] === 3) !== hasMetrics) refuse();
  return commands;
}
