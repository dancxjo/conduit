const VERSION = 4;
const MAX_BYTES = 131_072;
const MAX_NODES = 32;
const MAX_DEPTH = 8;
const MAX_KEY_BYTES = 32;
const MAX_TEXT_BYTES = 256;
const MAX_ACTIONS = 16;
const MAX_ACTION_ID_BYTES = 48;
const MAX_CONTROL_VALUE_BYTES = 65_536;
const MAX_EVENT_BYTES = MAX_CONTROL_VALUE_BYTES;
const MAX_EVENT_QUEUE = 8;
const MAX_EVENT_QUEUE_BYTES = 131_072;
const EVENT_HEADER_BYTES = 11;

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
  20: ["output", "success-status"], 21: ["output", "failure-status"],
  22: ["option", "option"],
});
const EVENTS = Object.freeze({ 1: "click", 2: "change", 3: "input", 4: "toggle", 5: "submit" });
const COMPONENT_IDENTITIES = Object.freeze(Object.fromEntries(
  Object.entries(COMPONENTS).map(([identity, [, kind]]) => [kind, Number(identity)]),
));
const EVENT_IDENTITIES = Object.freeze({ activate: 1, change: 2, input: 3, toggle: 4, submit: 5 });
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
    const valueLength = cursor.u32();
    const valueCapacity = cursor.u32();
    if (!(component in COMPONENTS)) refuse("unknown-component");
    if (keyLength === 0 || keyLength > MAX_KEY_BYTES || textLength > MAX_TEXT_BYTES) refuse("text-too-long");
    const hasValue = component === 15 || component === 16 || component === 17 || component === 22;
    if ((hasValue && (valueCapacity === 0 || valueCapacity > MAX_CONTROL_VALUE_BYTES || valueLength > valueCapacity))
      || (!hasValue && (valueCapacity !== 0 || valueLength !== 0))) refuse("invalid-control-value");
    if ((index === 0 && parent !== null) || (index !== 0 && (parent === null || parent >= index))) refuse("unknown-parent");
    if (action !== null && action >= actions.length) refuse("unknown-action");
    const key = cursor.text(keyLength);
    const text = cursor.text(textLength);
    const value = cursor.text(valueLength);
    if (keys.has(key)) refuse("duplicate-key");
    keys.add(key);
    const depth = parent === null ? 1 : depths[parent] + 1;
    if (depth > MAX_DEPTH) refuse("too-deep");
    depths.push(depth);
    return Object.freeze({ parent, component, action, key, text, value, valueCapacity });
  });
  if (cursor.offset !== encoded.length) refuse("malformed-encoding");
  return Object.freeze({ revision, actions: Object.freeze(actions), nodes: Object.freeze(nodes) });
}

export function encodeApplicationView(view) {
  if (!view || !Number.isSafeInteger(view.revision) || view.revision < 0 || view.revision > 0xffff_ffff) refuse("malformed-encoding");
  if (!Array.isArray(view.nodes) || !Array.isArray(view.actions)) refuse("malformed-encoding");
  const chunks = [];
  const push = (...bytes) => chunks.push(Uint8Array.of(...bytes));
  const header = new Uint8Array(7);
  header[0] = VERSION;
  new DataView(header.buffer).setUint32(1, view.revision, true);
  header[5] = view.nodes.length;
  header[6] = view.actions.length;
  chunks.push(header);
  for (const action of view.actions) {
    const kind = EVENT_IDENTITIES[action?.event];
    if (!kind) refuse("unknown-event");
    const encoded = new TextEncoder().encode(action.id ?? "");
    if (encoded.length === 0 || encoded.length > MAX_ACTION_ID_BYTES) refuse("action-id-too-long");
    push(kind, encoded.length);
    chunks.push(encoded);
  }
  for (const node of view.nodes) {
    const component = COMPONENT_IDENTITIES[node?.component];
    if (!component) refuse("unknown-component");
    const parent = node.parent === null ? 255 : node.parent;
    const action = node.action === null ? 255 : node.action;
    if (!Number.isInteger(parent) || parent < 0 || parent > 255 || !Number.isInteger(action) || action < 0 || action > 255) refuse("malformed-encoding");
    const key = new TextEncoder().encode(node.key ?? "");
    const content = new TextEncoder().encode(node.text ?? "");
    const value = new TextEncoder().encode(node.value ?? "");
    const valueCapacity = node.valueCapacity ?? 0;
    if (key.length === 0 || key.length > MAX_KEY_BYTES || content.length > MAX_TEXT_BYTES) refuse("text-too-long");
    const hasValue = component === 15 || component === 16 || component === 17 || component === 22;
    if (!Number.isSafeInteger(valueCapacity) || valueCapacity < 0 || valueCapacity > MAX_CONTROL_VALUE_BYTES
      || (hasValue && (valueCapacity === 0 || value.length > valueCapacity))
      || (!hasValue && (valueCapacity !== 0 || value.length !== 0))) refuse("invalid-control-value");
    const nodeHeader = new Uint8Array(14);
    nodeHeader.set([parent, component, action, key.length, content.length & 0xff, content.length >>> 8]);
    const headerView = new DataView(nodeHeader.buffer);
    headerView.setUint32(6, value.length, true);
    headerView.setUint32(10, valueCapacity, true);
    chunks.push(nodeHeader, key, content, value);
  }
  const length = chunks.reduce((total, chunk) => total + chunk.length, 0);
  if (length > MAX_BYTES) refuse("oversized-encoding");
  const encoded = new Uint8Array(length);
  let offset = 0;
  for (const chunk of chunks) { encoded.set(chunk, offset); offset += chunk.length; }
  decodeApplicationView(encoded);
  return encoded;
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
  new DataView(encoded.buffer).setUint32(7, value.length, true);
  encoded.set(action, EVENT_HEADER_BYTES);
  encoded.set(value, EVENT_HEADER_BYTES + action.length);
  return encoded;
}

export function manifestApplicationView(input, root, options = {}) {
  if (!(root instanceof Element)) throw new TypeError("application presentation requires an Element");
  const view = decodeApplicationView(input);
  const theme = options.theme ? decodeApplicationTheme(options.theme) : null;
  const capacity = options.eventCapacity ?? MAX_EVENT_QUEUE;
  const byteCapacity = options.eventByteCapacity ?? MAX_EVENT_QUEUE_BYTES;
  if (!Number.isInteger(capacity) || capacity < 1 || capacity > MAX_EVENT_QUEUE) refuse("queue-pressure");
  if (!Number.isInteger(byteCapacity) || byteCapacity < 1 || byteCapacity > MAX_EVENT_QUEUE_BYTES) refuse("queue-pressure");
  const queue = [];
  let queuedBytes = 0;
  let lastRefusal = null;
  const fragment = document.createDocumentFragment();
  const elements = [];
  for (const node of view.nodes) {
    const [tag, kind] = COMPONENTS[node.component];
    const element = document.createElement(tag);
    element.dataset.applicationKey = node.key;
    element.dataset.applicationComponent = kind;
    element.textContent = node.text;
    if (node.component === 15 || node.component === 16 || node.component === 17) {
      element.setAttribute("aria-label", node.text);
      element.value = node.value;
      if (node.component !== 16) element.maxLength = node.valueCapacity;
    }
    if (node.component === 22) element.value = node.value;
    if (node.component === 9 || node.component === 20 || node.component === 21) {
      element.setAttribute("aria-live", node.component === 21 ? "assertive" : "polite");
    }
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
        if (value.length > node.valueCapacity) {
          lastRefusal = "invalid-control-value";
          options.onRefusal?.(lastRefusal);
          return;
        }
        const encodedBytes = EVENT_HEADER_BYTES + new TextEncoder().encode(action.id).length + value.length;
        if (queue.length === capacity || queuedBytes + encodedBytes > byteCapacity) {
          lastRefusal = "queue-pressure";
          options.onRefusal?.(lastRefusal);
          return;
        }
        const pending = { revision: view.revision, action: action.id, kind: action.kind, value };
        const event = Object.freeze({ ...pending, encoded: encodeApplicationEvent(pending) });
        queue.push(event);
        queuedBytes += event.encoded.length;
        options.onEvent?.(event);
      });
    }
    if (node.component === 8 && node.action === null) element.disabled = true;
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
    nextEvent() {
      const event = queue.shift() ?? null;
      if (event) queuedBytes -= event.encoded.length;
      return event;
    },
    queuedEvents() { return queue.length; },
    queuedEventBytes() { return queuedBytes; },
    lastRefusal() { return lastRefusal; },
  });
}

export function createApplicationPresentationHost(scope = document) {
  const manifestations = new Map();
  const revisions = new Map();
  const refusals = new Map();
  const rootFor = (slot) => {
    if (typeof slot !== "string" || !/^[a-z][a-z0-9-]{0,47}$/.test(slot)) refuse("unknown-component");
    const matches = Array.from(scope.querySelectorAll("[data-application-slot]"))
      .filter((element) => element.dataset.applicationSlot === slot);
    if (matches.length !== 1) refuse("unknown-component");
    return matches[0];
  };
  return Object.freeze({
    present(slot, description, options = {}) {
      const encoded = description instanceof Uint8Array ? description : encodeApplicationView(description);
      const view = decodeApplicationView(encoded);
      const previous = revisions.get(slot) ?? 0;
      if (view.revision <= previous) refuse("stale-revision");
      revisions.set(slot, view.revision);
      refusals.delete(slot);
      const manifestation = manifestApplicationView(encoded, rootFor(slot), {
        eventCapacity: options.eventCapacity,
        eventByteCapacity: options.eventByteCapacity,
        theme: options.theme,
        onEvent(event) {
          if (revisions.get(slot) !== event.revision) {
            refusals.set(slot, "stale-revision");
            options.onRefusal?.("stale-revision");
            return;
          }
          options.onEvent?.(event);
        },
        onRefusal(code) { refusals.set(slot, code); options.onRefusal?.(code); },
      });
      manifestations.set(slot, manifestation);
      return Object.freeze({ revision: view.revision });
    },
    nextEvent(slot) { return manifestations.get(slot)?.nextEvent() ?? null; },
    queuedEvents(slot) { return manifestations.get(slot)?.queuedEvents() ?? 0; },
    queuedEventBytes(slot) { return manifestations.get(slot)?.queuedEventBytes() ?? 0; },
    lastRefusal(slot) { return refusals.get(slot) ?? manifestations.get(slot)?.lastRefusal() ?? null; },
  });
}

export const applicationPresentationLimits = Object.freeze({
  bytes: MAX_BYTES, nodes: MAX_NODES, depth: MAX_DEPTH, textBytes: MAX_TEXT_BYTES,
  actions: MAX_ACTIONS, resources: 0, controlValueBytes: MAX_CONTROL_VALUE_BYTES,
  eventBytes: MAX_EVENT_BYTES, eventQueue: MAX_EVENT_QUEUE, eventQueueBytes: MAX_EVENT_QUEUE_BYTES,
});
