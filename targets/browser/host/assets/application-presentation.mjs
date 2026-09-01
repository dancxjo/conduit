const VERSION = 1;
const MAX_BYTES = 16_384;
const MAX_NODES = 32;
const MAX_DEPTH = 8;
const MAX_KEY_BYTES = 32;
const MAX_TEXT_BYTES = 256;
const MAX_ACTIONS = 16;
const MAX_ACTION_ID_BYTES = 48;
const MAX_EVENT_BYTES = 512;
const MAX_EVENT_QUEUE = 8;
const EVENT_HEADER_BYTES = 9;

export class ApplicationPresentationRefusal extends Error {
  constructor(code) {
    super(`application presentation refused: ${code}`);
    this.name = "ApplicationPresentationRefusal";
    this.code = code;
  }
}

const refuse = (code) => { throw new ApplicationPresentationRefusal(code); };

class Cursor {
  constructor(encoded) { this.encoded = encoded; this.offset = 0; }
  bytes(length) {
    const end = this.offset + length;
    if (!Number.isSafeInteger(end) || end > this.encoded.length) refuse("malformed-encoding");
    const value = this.encoded.slice(this.offset, end);
    this.offset = end;
    return value;
  }
  byte() { return this.bytes(1)[0]; }
  u16() { const bytes = this.bytes(2); return bytes[0] | (bytes[1] << 8); }
  u32() { return new DataView(this.bytes(4).buffer).getUint32(0, true); }
  text(length) {
    try { return new TextDecoder("utf-8", { fatal: true }).decode(this.bytes(length)); }
    catch { refuse("malformed-encoding"); }
  }
}

const COMPONENTS = Object.freeze({
  1: ["div", "shell"], 2: ["header", "masthead"], 3: ["main", "main"],
  4: ["div", "stack"], 5: ["section", "panel"], 6: ["h1", "heading"],
  7: ["p", "paragraph"], 8: ["button", "button"], 9: ["output", "status"],
  10: ["details", "disclosure"], 11: ["div", "patchbay-canvas"],
  12: ["nav", "navigation"], 13: ["code", "code"], 14: ["div", "action-group"],
  15: ["input", "text-input"], 16: ["select", "select"], 17: ["textarea", "textarea"],
  18: ["table", "table"], 19: ["div", "grid"],
});
const EVENTS = Object.freeze({ 1: "click", 2: "change", 3: "input", 4: "toggle", 5: "submit" });
const THEME_ROLES = Object.freeze([
  "background", "reading-paper", "workbench-canvas", "bootstrap-surface", "surface",
  "structure-primary", "structure-secondary", "text-primary", "text-secondary", "emphasis",
  "focus", "warning", "failure", "success", "muted",
]);

export function decodeApplicationTheme(input) {
  const encoded = input instanceof Uint8Array ? input : new Uint8Array(input);
  const cursor = new Cursor(encoded);
  if (cursor.byte() !== VERSION) refuse("unsupported-version");
  const identityLength = cursor.byte();
  if (identityLength === 0 || identityLength > 64) refuse("text-too-long");
  const identity = cursor.text(identityLength);
  const tokens = {};
  for (const role of THEME_ROLES) {
    const [red, green, blue] = cursor.bytes(3);
    tokens[role] = `#${[red, green, blue].map((value) => value.toString(16).padStart(2, "0")).join("")}`;
  }
  if (cursor.offset !== encoded.length) refuse("malformed-encoding");
  return Object.freeze({ identity, tokens: Object.freeze(tokens) });
}

export function decodeApplicationView(input) {
  const encoded = input instanceof Uint8Array ? input : new Uint8Array(input);
  if (encoded.length > MAX_BYTES) refuse("oversized-encoding");
  const cursor = new Cursor(encoded);
  if (cursor.byte() !== VERSION) refuse("unsupported-version");
  const revision = cursor.u32();
  const nodeCount = cursor.byte();
  const actionCount = cursor.byte();
  if (nodeCount === 0) refuse("empty");
  if (nodeCount > MAX_NODES) refuse("too-many-nodes");
  if (actionCount > MAX_ACTIONS) refuse("too-many-actions");
  const actionIds = new Set();
  const actions = Array.from({ length: actionCount }, () => {
    const kind = cursor.byte();
    if (!(kind in EVENTS)) refuse("unknown-event");
    const length = cursor.byte();
    if (length === 0 || length > MAX_ACTION_ID_BYTES) refuse("action-id-too-long");
    const id = cursor.text(length);
    if (actionIds.has(id)) refuse("duplicate-action");
    actionIds.add(id);
    return Object.freeze({ id, kind, event: EVENTS[kind] });
  });
  const keys = new Set();
  const depths = [];
  const nodes = Array.from({ length: nodeCount }, (_, index) => {
    const rawParent = cursor.byte();
    const parent = rawParent === 255 ? null : rawParent;
    const component = cursor.byte();
    const rawAction = cursor.byte();
    const action = rawAction === 255 ? null : rawAction;
    const keyLength = cursor.byte();
    const textLength = cursor.u16();
    if (!(component in COMPONENTS)) refuse("unknown-component");
    if (keyLength === 0 || keyLength > MAX_KEY_BYTES || textLength > MAX_TEXT_BYTES) refuse("text-too-long");
    if ((index === 0 && parent !== null) || (index !== 0 && (parent === null || parent >= index))) refuse("unknown-parent");
    if (action !== null && action >= actions.length) refuse("unknown-action");
    const key = cursor.text(keyLength);
    const text = cursor.text(textLength);
    if (keys.has(key)) refuse("duplicate-key");
    keys.add(key);
    const depth = parent === null ? 1 : depths[parent] + 1;
    if (depth > MAX_DEPTH) refuse("too-deep");
    depths.push(depth);
    return Object.freeze({ parent, component, action, key, text });
  });
  if (cursor.offset !== encoded.length) refuse("malformed-encoding");
  return Object.freeze({ revision, actions: Object.freeze(actions), nodes: Object.freeze(nodes) });
}

function eventValue(event) {
  if (event.currentTarget instanceof HTMLInputElement || event.currentTarget instanceof HTMLTextAreaElement || event.currentTarget instanceof HTMLSelectElement) return event.currentTarget.value;
  return "";
}

export function encodeApplicationEvent(event) {
  const action = new TextEncoder().encode(event.action);
  const value = event.value instanceof Uint8Array ? event.value : new Uint8Array(event.value);
  if (action.length === 0 || action.length > MAX_ACTION_ID_BYTES) refuse("action-id-too-long");
  if (value.length > MAX_EVENT_BYTES) refuse("event-too-large");
  if (!(event.kind in EVENTS)) refuse("unknown-event");
  const encoded = new Uint8Array(EVENT_HEADER_BYTES + action.length + value.length);
  encoded[0] = VERSION;
  new DataView(encoded.buffer).setUint32(1, event.revision, true);
  encoded[5] = event.kind;
  encoded[6] = action.length;
  new DataView(encoded.buffer).setUint16(7, value.length, true);
  encoded.set(action, EVENT_HEADER_BYTES);
  encoded.set(value, EVENT_HEADER_BYTES + action.length);
  return encoded;
}

export function manifestApplicationView(input, root, options = {}) {
  if (!(root instanceof Element)) throw new TypeError("application presentation requires an Element");
  const view = decodeApplicationView(input);
  const theme = options.theme ? decodeApplicationTheme(options.theme) : null;
  const capacity = options.eventCapacity ?? MAX_EVENT_QUEUE;
  if (!Number.isInteger(capacity) || capacity < 1 || capacity > MAX_EVENT_QUEUE) refuse("queue-pressure");
  const queue = [];
  let lastRefusal = null;
  const fragment = document.createDocumentFragment();
  const elements = [];
  for (const node of view.nodes) {
    const [tag, kind] = COMPONENTS[node.component];
    const element = document.createElement(tag);
    element.dataset.applicationKey = node.key;
    element.dataset.applicationComponent = kind;
    element.textContent = node.text;
    if (node.component === 9) element.setAttribute("aria-live", "polite");
    if (node.component === 11) element.dataset.renderer = "patchbay";
    if (node.action !== null) {
      const action = view.actions[node.action];
      element.dataset.applicationAction = action.id;
      element.addEventListener(action.event, (browserEvent) => {
        const value = new TextEncoder().encode(eventValue(browserEvent));
        if (value.length > MAX_EVENT_BYTES) {
          lastRefusal = "event-too-large";
          options.onRefusal?.(lastRefusal);
          return;
        }
        if (queue.length === capacity) {
          lastRefusal = "queue-pressure";
          options.onRefusal?.(lastRefusal);
          return;
        }
        const pending = { revision: view.revision, action: action.id, kind: action.kind, value };
        const event = Object.freeze({ ...pending, encoded: encodeApplicationEvent(pending) });
        queue.push(event);
        options.onEvent?.(event);
      });
    }
    elements.push(element);
    if (node.parent === null) fragment.append(element);
    else elements[node.parent].append(element);
  }
  root.replaceChildren(fragment);
  root.dataset.applicationRevision = String(view.revision);
  if (theme) {
    root.dataset.applicationTheme = theme.identity;
    for (const [role, value] of Object.entries(theme.tokens)) {
      root.style.setProperty(`--conduit-${role}`, value);
    }
  }
  return Object.freeze({
    view,
    nextEvent() { return queue.shift() ?? null; },
    queuedEvents() { return queue.length; },
    lastRefusal() { return lastRefusal; },
  });
}

export const applicationPresentationLimits = Object.freeze({
  bytes: MAX_BYTES, nodes: MAX_NODES, depth: MAX_DEPTH, textBytes: MAX_TEXT_BYTES,
  actions: MAX_ACTIONS, resources: 0, eventBytes: MAX_EVENT_BYTES, eventQueue: MAX_EVENT_QUEUE,
});
