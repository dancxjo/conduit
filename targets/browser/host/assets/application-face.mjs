import { decodeGraphCanvas, renderGraphCanvas } from "./application-graph-canvas.mjs";
import { applicationLookLimits, decodeVisualLook } from "./application-look.mjs";

const VERSION = 10;
const RETIRED_VERSION = 9;
const MAX_BYTES = 131_072;
const MAX_NODES = 128;
const MAX_DEPTH = 8;
const MAX_KEY_BYTES = 32;
const MAX_TEXT_BYTES = 256;
const MAX_ACTIONS = 32;
const MAX_ACTION_ID_BYTES = 48;
const MAX_CONTROL_VALUE_BYTES = 65_536;
const MAX_EVENT_BYTES = MAX_CONTROL_VALUE_BYTES;
const MAX_EVENT_QUEUE = 8;
const MAX_EVENT_QUEUE_BYTES = 131_072;
const EVENT_HEADER_BYTES = 11;
const ROOT_IDENTITIES = new WeakMap();
const activeManifestations = new WeakMap();
let nextRootIdentity = 1;

export class ApplicationFaceRefusal extends Error {
  constructor(code) {
    super(`application face refused: ${code}`);
    this.name = "ApplicationFaceRefusal";
    this.code = code;
  }
}

const refuse = (code) => { throw new ApplicationFaceRefusal(code); };

function encodeExactText(value, maximum, boundCode) {
  if (typeof value !== "string") refuse("malformed-encoding");
  if (value.length > maximum) refuse(boundCode);
  if (!value.isWellFormed()) refuse("malformed-encoding");
  const bytes = new TextEncoder().encode(value);
  if (bytes.length > maximum) refuse(boundCode);
  return bytes;
}


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
  u32() {
    const bytes = this.bytes(4);
    return new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength).getUint32(0, true);
  }
  text(length) {
    try { return new TextDecoder("utf-8", { fatal: true, ignoreBOM: true }).decode(this.bytes(length)); }
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
  23: ["summary", "summary"],
  24: ["output", "warning-status"],
  25: ["section", "missing-evidence"], 26: ["section", "stale-evidence"],
  27: ["section", "refused-evidence"], 28: ["section", "failed-evidence"],
  29: ["section", "successful-evidence"], 30: ["dl", "definition-table"],
  31: ["div", "definition"], 32: ["pre", "code-block"],
  33: ["article", "artifact"],
  34: ["div", "form-field"], 35: ["label", "field-label"],
  36: ["p", "field-help"], 37: ["p", "field-error"],
  38: ["nav", "stepper"], 39: ["progress", "progress"],
  40: ["fieldset", "choice-group"], 41: ["legend", "choice-group-label"],
  42: ["label", "choice-option-label"], 43: ["input", "independent-choice"],
  44: ["input", "exclusive-choice"],
  45: ["a", "navigation-link"], 46: ["a", "link"],
  47: ["hr", "separator"],
});
const EVENTS = Object.freeze({ 1: "click", 2: "change", 3: "input", 4: "toggle", 5: "submit" });
const COMPONENT_IDENTITIES = Object.freeze(Object.fromEntries(
  Object.entries(COMPONENTS).map(([identity, [, kind]]) => [kind, Number(identity)]),
));
const EVENT_IDENTITIES = Object.freeze({ activate: 1, change: 2, input: 3, toggle: 4, submit: 5 });
const NODE_STATES = Object.freeze({ 1: "ready", 2: "busy", 3: "unavailable" });
const NODE_STATE_IDENTITIES = Object.freeze({ ready: 1, busy: 2, unavailable: 3 });
const EVIDENCE_DISPOSITIONS = Object.freeze({
  25: "missing", 26: "stale", 27: "refused", 28: "failed", 29: "succeeded",
});

function validProgress(value) {
  const match = /^(\d{1,5})\/(\d{1,5})$/.exec(value);
  if (!match) return false;
  const current = Number(match[1]);
  const total = Number(match[2]);
  return current <= 65_535 && total > 0 && total <= 65_535 && current <= total;
}

function validSameSiteLink(value) {
  return value.startsWith("/") && !value.startsWith("//")
    && new TextEncoder().encode(value).length <= 2_048
    && !/[\u0000-\u0020\u007f\\]/u.test(value);
}

const appliedLookProperties = new WeakMap();

export function decodeApplicationLook(input) {
  return decodeVisualLook(input, refuse);
}

export function decodeApplicationView(input) {
  const encoded = checkedBytes(input, MAX_BYTES, "oversized-encoding");
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
    const stateIdentity = cursor.byte();
    const rawAction = cursor.byte();
    const action = rawAction === 255 ? null : rawAction;
    const keyLength = cursor.byte();
    const textLength = cursor.u16();
    const valueLength = cursor.u32();
    const valueCapacity = cursor.u32();
    if (!(component in COMPONENTS)) refuse("unknown-component");
    if (!(stateIdentity in NODE_STATES)) refuse("malformed-encoding");
    const state = NODE_STATES[stateIdentity];
    if (keyLength === 0 || keyLength > MAX_KEY_BYTES || textLength > MAX_TEXT_BYTES) refuse("text-too-long");
    const hasValue = (component === 11 && valueLength > 0) || component === 15 || component === 16 || component === 17 || component === 22 || component === 31 || component === 32 || component === 38 || component === 39 || component === 43 || component === 44 || component === 45 || component === 46;
    if ((hasValue && (valueCapacity === 0 || valueCapacity > MAX_CONTROL_VALUE_BYTES || valueLength > valueCapacity))
      || (!hasValue && component !== 12 && (valueCapacity !== 0 || valueLength !== 0))) refuse("invalid-control-value");
    if (component === 12 && ((valueLength > 0 && (valueCapacity === 0 || valueCapacity > MAX_CONTROL_VALUE_BYTES || valueLength > valueCapacity)) || (valueLength === 0 && valueCapacity !== 0))) refuse("invalid-control-value");
    if ((index === 0 && parent !== null) || (index !== 0 && (parent === null || parent >= index))) refuse("unknown-parent");
    if (action !== null && action >= actions.length) refuse("unknown-action");
    if ((component === 45 || component === 46) && action !== null) refuse("invalid-control-value");
    if ((component === 43 || component === 44) && action !== null && actions[action].event !== "change") refuse("invalid-control-value");
    const stateful = component === 8 || component === 15 || component === 16 || component === 17 || component === 43 || component === 44;
    if (state !== "ready" && (!stateful || action !== null)) refuse("invalid-node-state");
    const key = cursor.text(keyLength);
    const text = cursor.text(textLength);
    const value = cursor.text(valueLength);
    if ([12, 14, 39, 40, 41, 42, 43, 44, 45, 46, 47].includes(component) && text.length === 0) refuse("invalid-control-value");
    if ((component === 38 || component === 39) && !validProgress(value)) refuse("invalid-control-value");
    if (component === 38 && value.startsWith("0/")) refuse("invalid-control-value");
    if (keys.has(key)) refuse("duplicate-key");
    keys.add(key);
    const depth = parent === null ? 1 : depths[parent] + 1;
    if (depth > MAX_DEPTH) refuse("too-deep");
    depths.push(depth);
    if (component === 11 && value) decodeGraphCanvas(value);
    return Object.freeze({ parent, component, state, action, key, text, value, valueCapacity });
  });
  for (const [index, node] of nodes.entries()) {
    if (node.component === 12 && node.value && !nodes.some((child, childIndex) => childIndex > index && child.parent === index && (child.component === 8 || child.component === 45) && child.key === node.value)) refuse("invalid-control-value");
    const children = nodes.filter((child) => child.parent === index);
    if (node.component === 16 && node.value && !children.some(child => child.component === 22 && child.value === node.value)) refuse("invalid-control-value");
    if (node.component === 34) {
      const count = (component) => children.filter((child) => child.component === component).length;
      if (count(35) !== 1 || count(36) !== 1 || count(37) > 1 || count(15) + count(16) + count(17) !== 1) refuse("invalid-control-value");
    }
    if ((node.component === 35 || node.component === 36 || node.component === 37)
      && (node.parent === null || nodes[node.parent].component !== 34)) refuse("invalid-control-value");
    if (node.component === 22 && (node.parent === null || nodes[node.parent].component !== 16)) refuse("invalid-control-value");
    if (node.component === 38 && children.filter((child) => child.component === 8).length !== Number(node.value.split("/")[1])) refuse("invalid-control-value");
    if (node.component === 40) {
      const labels = children.filter((child) => child.component === 42);
      const choices = labels.flatMap((label) => {
        const labelIndex = nodes.indexOf(label);
        return nodes.filter((child) => child.parent === labelIndex && (child.component === 43 || child.component === 44));
      });
      const kinds = new Set(choices.map(child => child.component));
      if (children.filter((child) => child.component === 41).length !== 1 || labels.length === 0 || kinds.size !== 1
        || choices.filter(child => child.component === 44 && child.value === "true").length > 1) refuse("invalid-control-value");
    }
    if (node.component === 41 && (node.parent === null || nodes[node.parent].component !== 40)) refuse("invalid-control-value");
    if (node.component === 42 && (node.parent === null || nodes[node.parent].component !== 40 || children.filter((child) => child.component === 43 || child.component === 44).length !== 1)) refuse("invalid-control-value");
    if ((node.component === 43 || node.component === 44) && (!['true', 'false'].includes(node.value) || node.parent === null || nodes[node.parent].component !== 42)) refuse("invalid-control-value");
    if (node.component === 45 && (!['home', 'tour', 'creche', 'patchbay', 'source'].includes(node.value) || node.parent === null || nodes[node.parent].component !== 12)) refuse("invalid-control-value");
    if (node.component === 46 && !validSameSiteLink(node.value)) refuse("invalid-control-value");
    if (node.component === 47 && children.length !== 0) refuse("invalid-control-value");
  }
  if (cursor.offset !== encoded.length) refuse("malformed-encoding");
  return Object.freeze({ revision, actions: Object.freeze(actions), nodes: Object.freeze(nodes) });
}

export function encodeApplicationView(view) {
  const revision = view?.revision;
  const nodes = view?.nodes;
  const actions = view?.actions;
  if (!Number.isSafeInteger(revision) || revision < 0 || revision > 0xffff_ffff) refuse("malformed-encoding");
  if (!Array.isArray(nodes) || !Array.isArray(actions)) refuse("malformed-encoding");
  const nodeCount = nodes.length;
  const actionCount = actions.length;
  if (nodeCount === 0) refuse("empty");
  if (nodeCount > MAX_NODES) refuse("too-many-nodes");
  if (actionCount > MAX_ACTIONS) refuse("too-many-actions");
  const chunks = [];
  let length = 0;
  const append = (...values) => {
    for (const chunk of values) {
      if (chunk.length > MAX_BYTES - length) refuse("oversized-encoding");
      chunks.push(chunk);
      length += chunk.length;
    }
  };
  const push = (...bytes) => append(Uint8Array.of(...bytes));
  const header = new Uint8Array(7);
  header[0] = VERSION;
  new DataView(header.buffer).setUint32(1, revision, true);
  header[5] = nodeCount;
  header[6] = actionCount;
  append(header);
  for (let index = 0; index < actionCount; index++) {
    const action = actions[index];
    const kind = EVENT_IDENTITIES[action?.event];
    if (!kind) refuse("unknown-event");
    const encoded = encodeExactText(action.id ?? "", MAX_ACTION_ID_BYTES, "action-id-too-long");
    if (encoded.length === 0 || encoded.length > MAX_ACTION_ID_BYTES) refuse("action-id-too-long");
    push(kind, encoded.length);
    append(encoded);
  }
  for (let index = 0; index < nodeCount; index++) {
    const node = nodes[index];
    const component = COMPONENT_IDENTITIES[node?.component];
    if (!component) refuse("unknown-component");
    const state = NODE_STATE_IDENTITIES[node?.state ?? "ready"];
    if (!state) refuse("malformed-encoding");
    const parent = node.parent === null ? 255 : node.parent;
    const action = node.action === null ? 255 : node.action;
    if (!Number.isInteger(parent) || parent < 0 || parent > 255 || !Number.isInteger(action) || action < 0 || action > 255) refuse("malformed-encoding");
    const key = encodeExactText(node.key ?? "", MAX_KEY_BYTES, "text-too-long");
    const content = encodeExactText(node.text ?? "", MAX_TEXT_BYTES, "text-too-long");
    const value = encodeExactText(node.value ?? "", MAX_CONTROL_VALUE_BYTES, "invalid-control-value");
    const valueCapacity = node.valueCapacity ?? 0;
    if (key.length === 0 || key.length > MAX_KEY_BYTES || content.length > MAX_TEXT_BYTES) refuse("text-too-long");
    const hasValue = (component === 11 && value.length > 0) || component === 15 || component === 16 || component === 17 || component === 22 || component === 31 || component === 32 || component === 38 || component === 39 || component === 43 || component === 44 || component === 45 || component === 46;
    if (!Number.isSafeInteger(valueCapacity) || valueCapacity < 0 || valueCapacity > MAX_CONTROL_VALUE_BYTES
      || (hasValue && (valueCapacity === 0 || value.length > valueCapacity))
      || (!hasValue && component !== 12 && (valueCapacity !== 0 || value.length !== 0))) refuse("invalid-control-value");
    if (component === 12 && ((value.length > 0 && (valueCapacity === 0 || valueCapacity > MAX_CONTROL_VALUE_BYTES || value.length > valueCapacity)) || (value.length === 0 && valueCapacity !== 0))) refuse("invalid-control-value");
    if ((component === 38 || component === 39) && !validProgress(new TextDecoder().decode(value))) refuse("invalid-control-value");
    if (component === 38 && Number(new TextDecoder().decode(value).split("/")[0]) === 0) refuse("invalid-control-value");
    const stateful = component === 8 || component === 15 || component === 16 || component === 17 || component === 43 || component === 44;
    if (state !== NODE_STATE_IDENTITIES.ready && (!stateful || action !== 255)) refuse("invalid-node-state");
    const nodeHeader = new Uint8Array(15);
    nodeHeader.set([parent, component, state, action, key.length, content.length & 0xff, content.length >>> 8]);
    const headerView = new DataView(nodeHeader.buffer);
    headerView.setUint32(7, value.length, true);
    headerView.setUint32(11, valueCapacity, true);
    append(nodeHeader, key, content, value);
  }
  const encoded = new Uint8Array(length);
  let offset = 0;
  for (const chunk of chunks) { encoded.set(chunk, offset); offset += chunk.length; }
  decodeApplicationView(encoded);
  return encoded;
}

function eventValue(event) {
  if (event.currentTarget instanceof HTMLInputElement
    && (event.currentTarget.type === "checkbox" || event.currentTarget.type === "radio")) {
    return event.currentTarget.checked ? "true" : "false";
  }
  if (event.currentTarget instanceof HTMLInputElement || event.currentTarget instanceof HTMLTextAreaElement || event.currentTarget instanceof HTMLSelectElement) return event.currentTarget.value;
  return "";
}

const NAVIGATION_DESTINATIONS = Object.freeze({
  home: "/conduit",
  tour: "/conduit/workspace/",
  creche: "/conduit/workspace/",
  patchbay: "/conduit/patchbay/",
  source: "https://github.com/dancxjo/conduit",
});

export function browserDestinationHref(destination) {
  const href = NAVIGATION_DESTINATIONS[destination];
  if (!href) refuse("invalid-control-value");
  return href;
}

function rootIdentity(root) {
  let identity = ROOT_IDENTITIES.get(root);
  if (identity) return identity;
  if (!Number.isSafeInteger(nextRootIdentity)) refuse("malformed-encoding");
  identity = `application-${nextRootIdentity}`;
  nextRootIdentity += 1;
  ROOT_IDENTITIES.set(root, identity);
  return identity;
}

function restoreInteraction(root, snapshot, render) {
  if (!snapshot) return;
  let preferred = Array.from(root.querySelectorAll("[data-application-key]"))
    .find(element => element.dataset.applicationKey === snapshot.key);
  if (preferred && snapshot.subject) preferred = Array.from(preferred.querySelectorAll("[data-subject-identity]"))
    .find(element => element.dataset.subjectIdentity === snapshot.subject);
  const candidates = new Set([preferred,
    ...root.querySelectorAll('[tabindex="0"]'),
    ...root.querySelectorAll('button, input, select, textarea, a[href], summary')]);
  let target;
  for (const candidate of candidates) {
    if (!(candidate instanceof HTMLElement) && !(candidate instanceof SVGElement)) continue;
    if (!candidate.isConnected || !root.contains(candidate) || !controlAvailable(candidate)) continue;
    candidate.focus();
    // Focus handlers can synchronously present a newer view. Leave its focus
    // alone and let the superseded caller refuse before publishing its handle.
    if (activeManifestations.get(root) !== render) return;
    // A retained key may now name plain text or a non-focusable container.
    if (candidate.ownerDocument.activeElement === candidate) { target = candidate; break; }
  }
  if (!target) {
    root.tabIndex = -1;
    root.focus();
    target = root;
  }
  const textSelection = target instanceof HTMLTextAreaElement
    || (target instanceof HTMLInputElement && ["text", "search", "tel", "url", "password"].includes(target.type));
  if (target === preferred && textSelection && snapshot.selection) {
    const end = target.value.length;
    target.setSelectionRange(Math.min(snapshot.selection.start, end), Math.min(snapshot.selection.end, end), snapshot.selection.direction);
  }
}

function controlAvailable(control) {
  if (control.matches(":disabled")) return false;
  for (let ancestor = control; ancestor;
    ancestor = ancestor.assignedSlot ?? ancestor.parentElement ?? ancestor.getRootNode().host) {
    if (ancestor.inert || ancestor.hidden) return false;
  }
  return true;
}

function installRovingFocus(container, selectedKey, selectedIndex) {
  // Navigation selections and step indices name direct children in the view.
  // Descendant groups own their controls and must retain their own tab entry.
  const controls = Array.from(container.children).filter(control => control.matches("button"));
  const available = controls.filter(controlAvailable);
  if (available.length === 0) return;
  const selected = selectedKey ? controls.find(control => control.dataset.applicationKey === selectedKey) : controls[selectedIndex];
  const current = available.includes(selected) ? selected : available[0];
  controls.forEach(control => { control.tabIndex = control === current ? 0 : -1; });
  container.addEventListener("keydown", (event) => {
    const available = controls.filter(control => control.isConnected && container.contains(control) && controlAvailable(control));
    const focused = available.indexOf(document.activeElement);
    if (focused < 0) return;
    let next = focused;
    if (event.key === "ArrowRight" || event.key === "ArrowDown") next = Math.min(focused + 1, available.length - 1);
    else if (event.key === "ArrowLeft" || event.key === "ArrowUp") next = Math.max(focused - 1, 0);
    else if (event.key === "Home") next = 0;
    else if (event.key === "End") next = available.length - 1;
    else return;
    event.preventDefault();
    controls.forEach(control => { control.tabIndex = control === available[next] ? 0 : -1; });
    available[next].focus();
  });
}

function checkedBytes(value, maximum, boundCode) {
  if (value instanceof Uint8Array || value instanceof ArrayBuffer) {
    if (value.byteLength > maximum) refuse(boundCode);
    return value instanceof Uint8Array ? value : new Uint8Array(value);
  }
  if (!Array.isArray(value)) refuse("malformed-encoding");
  const length = value.length;
  if (length > maximum) refuse(boundCode);
  const bytes = new Uint8Array(length);
  for (let index = 0; index < length; index++) {
    const byte = value[index];
    if (!Object.hasOwn(value, index) || !Number.isInteger(byte) || byte < 0 || byte > 255) refuse("malformed-encoding");
    bytes[index] = byte;
  }
  return bytes;
}

export function encodeApplicationEvent(event) {
  if (!event || !Number.isSafeInteger(event.revision) || event.revision < 0 || event.revision > 0xffff_ffff) refuse("malformed-encoding");
  if (!Number.isInteger(event.kind) || !Object.hasOwn(EVENTS, event.kind)) refuse("unknown-event");
  const action = encodeExactText(event.action, MAX_ACTION_ID_BYTES, "action-id-too-long");
  const value = checkedBytes(event.value, MAX_EVENT_BYTES, "event-too-large");
  if (action.length === 0 || action.length > MAX_ACTION_ID_BYTES) refuse("action-id-too-long");
  if (value.length > MAX_EVENT_BYTES) refuse("event-too-large");
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

function copyApplicationEvent(event) {
  return Object.freeze({ ...event,
    value: new Uint8Array(event.value), encoded: new Uint8Array(event.encoded) });
}

export function manifestApplicationView(input, root, options = {}) {
  if (!(root instanceof Element)) throw new TypeError("application face requires an Element");
  const previousManifestation = activeManifestations.get(root);
  const view = decodeApplicationView(input);
  const active = root.contains(document.activeElement) ? document.activeElement : null;
  const activeNode = active?.closest?.("[data-application-key]");
  const interaction = activeNode ? {
    key: activeNode.dataset.applicationKey,
    subject: active?.closest?.("[data-subject-identity]")?.dataset.subjectIdentity,
    selection: active instanceof HTMLInputElement || active instanceof HTMLTextAreaElement ? {
      start: active.selectionStart ?? 0, end: active.selectionEnd ?? 0, direction: active.selectionDirection ?? "none",
    } : null,
  } : null;
  const theme = options.look ? decodeApplicationLook(options.look) : null;
  const capacity = options.eventCapacity ?? MAX_EVENT_QUEUE;
  const byteCapacity = options.eventByteCapacity ?? MAX_EVENT_QUEUE_BYTES;
  if (!Number.isInteger(capacity) || capacity < 1 || capacity > MAX_EVENT_QUEUE) refuse("queue-pressure");
  if (!Number.isInteger(byteCapacity) || byteCapacity < 1 || byteCapacity > MAX_EVENT_QUEUE_BYTES) refuse("queue-pressure");
  const render = {};
  const queue = [];
  let queuedBytes = 0;
  let lastRefusal = null;
  const fragment = document.createDocumentFragment();
  const elements = [];
  const actionEvents = new WeakMap();
  const choiceValues = new Map();
  const nodesByKey = new Map();
  const valueComponents = new Set([11, 15, 16, 17, 43, 44]);
  // Wire events name the action, not an originating node. All of its bindings
  // must therefore admit the argument, as in portable ApplicationEvent admission.
  const actionBindings = view.actions.map((_, index) => view.nodes.filter(node => node.action === index));
  const actionValueCapacities = actionBindings.map(bindings => Math.min(...bindings.map(node =>
    valueComponents.has(node.component) ? node.valueCapacity : 0)));
  const requestAction = (node, value = "", control = null) => {
    if (activeManifestations.get(root) !== render) {
      lastRefusal = "stale-revision";
      options.onRefusal?.(lastRefusal);
      return null;
    }
    if (control) {
      if (!control.isConnected || !root.contains(control) || !controlAvailable(control)) {
        lastRefusal = "unavailable-action";
        options.onRefusal?.(lastRefusal);
        return null;
      }
    }
    if (node.state === "busy") {
      lastRefusal = "action-busy";
      options.onRefusal?.(lastRefusal);
      return null;
    }
    if (node.state === "unavailable" || node.action === null) {
      lastRefusal = "unavailable-action";
      options.onRefusal?.(lastRefusal);
      return null;
    }
    const action = view.actions[node.action];
    const valueCapacity = actionValueCapacities[node.action];
    // Every UTF-16 code unit requires at least one UTF-8 byte. Reject an
    // impossible fit before allocating the encoded copy; retain the byte check
    // below for multibyte text and byte inputs.
    if (typeof value === "string" && (value.length > MAX_EVENT_BYTES || value.length > valueCapacity)) {
      lastRefusal = value.length > MAX_EVENT_BYTES ? "event-too-large" : "invalid-control-value";
      options.onRefusal?.(lastRefusal);
      return null;
    }
    const encodedValue = typeof value === "string" ? new TextEncoder().encode(value) : value;
    if (!(encodedValue instanceof Uint8Array)) refuse("malformed-encoding");
    if (encodedValue.length > MAX_EVENT_BYTES) {
      lastRefusal = "event-too-large";
      options.onRefusal?.(lastRefusal);
      return null;
    }
    if (encodedValue.length > valueCapacity) {
      lastRefusal = "invalid-control-value";
      options.onRefusal?.(lastRefusal);
      return null;
    }
    let text;
    try { text = typeof value === "string" ? (value.isWellFormed() ? value : undefined)
      : new TextDecoder("utf-8", { fatal: true, ignoreBOM: true }).decode(encodedValue); }
    catch { text = undefined; }
    for (const boundNode of actionBindings[node.action]) {
      const choices = choiceValues.get(boundNode);
      if (text === undefined || (choices && !choices.has(text))
        || ([43, 44].includes(boundNode.component) && text !== "true" && text !== "false")) {
        lastRefusal = "invalid-control-value";
        options.onRefusal?.(lastRefusal);
        return null;
      }
    }
    const encodedBytes = EVENT_HEADER_BYTES + new TextEncoder().encode(action.id).length + encodedValue.length;
    if (queue.length === capacity || queuedBytes + encodedBytes > byteCapacity) {
      lastRefusal = "queue-pressure";
      options.onRefusal?.(lastRefusal);
      return null;
    }
    const pending = { revision: view.revision, action: action.id, kind: action.kind, value: new Uint8Array(encodedValue) };
    const event = Object.freeze({ ...pending, encoded: encodeApplicationEvent(pending) });
    queue.push(event);
    queuedBytes += event.encoded.length;
    options.onEvent?.(copyApplicationEvent(event));
    return copyApplicationEvent(event);
  };
  for (const [nodeIndex, node] of view.nodes.entries()) {
    if (node.component === 16) {
      choiceValues.set(node, new Set(view.nodes.filter(child => child.parent === nodeIndex && child.component === 22).map(child => child.value)));
    }
    if (node.component === 11 && node.value) {
      const graph = decodeGraphCanvas(node.value);
      choiceValues.set(node, new Set([...graph.nodes, ...graph.ports, ...graph.cords].map(subject => subject.identity)));
    }
    const [tag, kind] = COMPONENTS[node.component];
    const element = document.createElement(tag);
    if (node.component === 8) element.type = "button";
    element.dataset.applicationKey = node.key;
    element.dataset.applicationComponent = kind;
    if (node.component === 10) {
      const hasSummary = view.nodes.some((child) => child.parent === nodeIndex && child.component === 23);
      if (!hasSummary) {
        const summary = document.createElement("summary");
        summary.textContent = node.text;
        summary.dataset.applicationDisclosureSummary = node.key;
        element.append(summary);
      }
    } else if (node.component === 31) {
      const term = document.createElement("dt");
      term.textContent = node.text;
      const value = document.createElement("dd");
      value.textContent = node.value;
      element.append(term, value);
    } else if (node.component === 32) {
      const code = document.createElement("code");
      code.textContent = node.value;
      element.dataset.applicationLanguage = node.text;
      element.append(code);
    } else if (node.component === 14) {
      element.setAttribute("role", "group");
      if (node.text) element.setAttribute("aria-label", node.text);
    } else if (node.component === 30) {
      element.setAttribute("aria-label", node.text);
    } else if (node.component === 39) {
      const [current, total] = node.value.split("/").map(Number);
      element.value = current;
      element.max = total;
      element.setAttribute("aria-label", node.text);
      element.dataset.applicationCurrent = String(current);
      element.dataset.applicationTotal = String(total);
    } else if (node.component === 38) {
      const [current, total] = node.value.split("/").map(Number);
      element.setAttribute("aria-label", node.text);
      element.dataset.applicationCurrent = String(current);
      element.dataset.applicationTotal = String(total);
    } else if (node.component === 12) {
      element.setAttribute("aria-label", node.text);
      element.dataset.applicationCurrent = node.value;
    } else if (node.component === 40) {
      element.dataset.applicationChoiceName = node.text;
    } else if (node.component === 43 || node.component === 44) {
      element.type = node.component === 43 ? "checkbox" : "radio";
      element.name = `${options.choiceScope ?? rootIdentity(root)}-${view.nodes[view.nodes[node.parent].parent].text}`;
      element.value = node.text;
      element.checked = node.value === "true";
    } else if (node.component === 45) {
      element.href = browserDestinationHref(node.value);
      element.textContent = node.text;
      if (node.value === "home") element.setAttribute("aria-label", "Conduit home");
    } else if (node.component === 46) {
      element.href = new URL(node.value, document.baseURI).href;
      element.textContent = node.text;
    } else if (node.component === 47) {
      element.setAttribute("aria-label", node.text);
    } else if (node.component === 33 || node.component in EVIDENCE_DISPOSITIONS) {
      const title = document.createElement("h3");
      title.textContent = node.text;
      element.append(title);
    } else {
      element.textContent = node.text;
    }
    if (node.component === 8 || node.component === 15 || node.component === 16 || node.component === 17 || node.component === 43 || node.component === 44) {
      element.dataset.applicationAvailability = node.state;
    }
    if (node.component === 6) element.tabIndex = -1;
    if (node.component === 15 || node.component === 16 || node.component === 17) {
      element.setAttribute("aria-label", node.text);
      element.value = node.value;
      // Native text controls sanitize line endings. Refuse before publication
      // when the admitted value cannot be represented exactly by this control.
      if (node.component !== 16 && element.value !== node.value) refuse("invalid-control-value");
      if (node.component !== 16) element.maxLength = node.valueCapacity;
    }
    if (node.component === 22) element.value = node.value;
    if (node.component === 9 || node.component === 20 || node.component === 21 || node.component === 24) {
      const severity = node.component === 20 ? "success" : node.component === 21 ? "failure" : node.component === 24 ? "warning" : "ordinary";
      element.dataset.applicationStatus = severity;
      element.setAttribute("role", node.component === 21 ? "alert" : "status");
      element.setAttribute("aria-live", node.component === 21 ? "assertive" : "polite");
      const existingHostState = document.getElementById("host-state");
      if (node.key === "product-status" && (!existingHostState || root.contains(existingHostState))) {
        element.id = "host-state";
      }
    }
    if (node.component in EVIDENCE_DISPOSITIONS) {
      const disposition = EVIDENCE_DISPOSITIONS[node.component];
      element.dataset.applicationEvidence = disposition;
      element.setAttribute("role", disposition === "failed" ? "alert" : "status");
      element.setAttribute("aria-live", disposition === "failed" ? "assertive" : "polite");
    }
    if (node.component === 11) {
      element.dataset.renderer = "patchbay";
      if (node.value) renderGraphCanvas(element, node.value, (identity, control) => requestAction(node, identity, control));
    }
    if (node.action !== null) {
      const action = view.actions[node.action];
      element.dataset.applicationAction = action.id;
      actionEvents.set(element, action.event);
      // Populated graph canvases return exact subject identities through their
      // selection callback; a second generic listener would add an empty action.
      if (node.component !== 11 || !node.value) element.addEventListener(action.event, (browserEvent) => {
        // A child binding owns its event; bubbling must not invent a second
        // semantic action on an ancestor. Decorative descendants still route here.
        if (browserEvent.composedPath().find(node => actionEvents.get(node) === browserEvent.type) !== element) return;
        requestAction(node, eventValue(browserEvent), element);
      });
    }
    if (node.state === "busy") element.setAttribute("aria-busy", "true");
    if ((node.component === 8 || node.component === 15 || node.component === 16 || node.component === 17 || node.component === 43 || node.component === 44)
      && (node.action === null || node.state !== "ready")) element.disabled = true;
    elements.push(element);
    nodesByKey.set(node.key, node);
    if (node.parent === null) fragment.append(element);
    else if (node.component === 43 || node.component === 44) elements[node.parent].prepend(element);
    else elements[node.parent].append(element);
  }
  const instance = rootIdentity(root);
  for (const [index, node] of view.nodes.entries()) {
    if (node.component === 34) {
      const children = view.nodes.map((child, childIndex) => ({ child, childIndex })).filter(({ child }) => child.parent === index);
      const control = children.find(({ child }) => child.component === 15 || child.component === 16 || child.component === 17);
      const label = children.find(({ child }) => child.component === 35);
      const help = children.find(({ child }) => child.component === 36);
      const error = children.find(({ child }) => child.component === 37);
      if (!control || !label || !help) refuse("malformed-encoding");
      const controlElement = elements[control.childIndex];
      const controlId = `${instance}-control-${control.childIndex}`;
      controlElement.id = controlId;
      controlElement.removeAttribute("aria-label");
      elements[label.childIndex].htmlFor = controlId;
      const descriptions = [];
      for (const described of [help, error]) {
        if (!described) continue;
        const id = `${instance}-description-${described.childIndex}`;
        elements[described.childIndex].id = id;
        descriptions.push(id);
      }
      controlElement.setAttribute("aria-describedby", descriptions.join(" "));
      if (error) {
        controlElement.setAttribute("aria-invalid", "true");
        controlElement.setAttribute("aria-errormessage", elements[error.childIndex].id);
        elements[error.childIndex].setAttribute("role", "alert");
      }
    } else if (node.component === 12) {
      installRovingFocus(elements[index], node.value, -1);
      for (const [childIndex, child] of view.nodes.entries()) {
        if (child.parent === index && (child.component === 8 || child.component === 45) && child.key === node.value) {
          elements[childIndex].setAttribute("aria-current", "page");
        }
      }
    } else if (node.component === 38) {
      installRovingFocus(elements[index], "", Number(node.value.split("/")[0]) - 1);
      const current = view.nodes
        .map((child, childIndex) => ({ child, childIndex }))
        .filter(({ child }) => child.parent === index && child.component === 8)[Number(node.value.split("/")[0]) - 1];
      if (current) elements[current.childIndex].setAttribute("aria-current", "step");
    } else if (node.component === 16) {
      elements[index].value = node.value;
      if (elements[index].value !== node.value) refuse("invalid-control-value");
    }
  }
  if (activeManifestations.get(root) !== previousManifestation) refuse("stale-revision");
  root.replaceChildren(fragment);
  activeManifestations.set(root, render);
  root.dataset.applicationRevision = String(view.revision);
  for (const role of appliedLookProperties.get(root) ?? []) {
    root.style.removeProperty(`--conduit-${role}`);
  }
  if (theme) {
    appliedLookProperties.set(root, Object.keys(theme.tokens));
    root.dataset.applicationLook = theme.identity;
    for (const [role, value] of Object.entries(theme.tokens)) {
      root.style.setProperty(`--conduit-${role}`, value);
    }
  } else {
    appliedLookProperties.delete(root);
    delete root.dataset.applicationLook;
  }
  restoreInteraction(root, interaction, render);
  if (activeManifestations.get(root) !== render) refuse("stale-revision");
  return Object.freeze({
    view,
    nextEvent() {
      const event = queue.shift() ?? null;
      if (event) queuedBytes -= event.encoded.length;
      return event ? copyApplicationEvent(event) : null;
    },
    queuedEvents() { return queue.length; },
    queuedEventBytes() { return queuedBytes; },
    lastRefusal() { return lastRefusal; },
    requestAction(key, value = "") {
      const node = nodesByKey.get(key);
      if (!node) refuse("unknown-component");
      return requestAction(node, value);
    },
  });
}

export function createApplicationFaceHost(scope = document) {
  const manifestations = new Map();
  const revisions = new Map();
  const refusals = new Map();
  const choiceScope = rootIdentity(scope);
  const rootFor = (slot) => {
    if (typeof slot !== "string" || !/^[a-z][a-z0-9-]{0,47}$/.test(slot)) refuse("unknown-component");
    const matches = Array.from(scope.querySelectorAll("[data-application-slot]"))
      .filter((element) => element.dataset.applicationSlot === slot);
    if (matches.length !== 1) refuse("unknown-component");
    return matches[0];
  };
  return Object.freeze({
    present(slot, description, options = {}) {
      const encoded = description instanceof Uint8Array
        ? new Uint8Array(checkedBytes(description, MAX_BYTES, "oversized-encoding"))
        : encodeApplicationView(description);
      const view = decodeApplicationView(encoded);
      // Read caller-controlled options before the final revision check. Their
      // getters may synchronously commit another presentation of this slot.
      const { eventCapacity, eventByteCapacity, look, onEvent, onRefusal } = options;
      const previous = revisions.get(slot) ?? 0;
      if (view.revision <= previous) refuse("stale-revision");
      const manifestation = manifestApplicationView(encoded, rootFor(slot), {
        eventCapacity,
        eventByteCapacity,
        choiceScope,
        look,
        onEvent(event) {
          if (revisions.get(slot) !== event.revision) {
            refusals.set(slot, "stale-revision");
            onRefusal?.("stale-revision");
            return;
          }
          onEvent?.(event);
        },
        onRefusal(code) { refusals.set(slot, code); onRefusal?.(code); },
      });
      revisions.set(slot, view.revision);
      refusals.delete(slot);
      manifestations.set(slot, manifestation);
      return Object.freeze({ revision: view.revision });
    },
    nextEvent(slot) { return manifestations.get(slot)?.nextEvent() ?? null; },
    queuedEvents(slot) { return manifestations.get(slot)?.queuedEvents() ?? 0; },
    queuedEventBytes(slot) { return manifestations.get(slot)?.queuedEventBytes() ?? 0; },
    lastRefusal(slot) { return refusals.get(slot) ?? manifestations.get(slot)?.lastRefusal() ?? null; },
    requestAction(slot, key, value = "") {
      const manifestation = manifestations.get(slot);
      if (!manifestation) refuse("unknown-component");
      return manifestation.requestAction(key, value);
    },
  });
}

export const applicationFaceLimits = Object.freeze({
  version: VERSION, retiredVersion: RETIRED_VERSION,
  lookVersion: applicationLookLimits.version, lookBytes: applicationLookLimits.bytes,
  bytes: MAX_BYTES, nodes: MAX_NODES, depth: MAX_DEPTH, textBytes: MAX_TEXT_BYTES,
  actions: MAX_ACTIONS, resources: 0, controlValueBytes: MAX_CONTROL_VALUE_BYTES,
  eventBytes: MAX_EVENT_BYTES, eventQueue: MAX_EVENT_QUEUE, eventQueueBytes: MAX_EVENT_QUEUE_BYTES,
});
