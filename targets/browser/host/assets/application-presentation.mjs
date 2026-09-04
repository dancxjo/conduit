import { applicationThemeLimits, decodeTheme } from "./application-theme.mjs";

const VERSION = 8;
const RETIRED_VERSION = 7;
const MAX_BYTES = 131_072;
const MAX_NODES = 40;
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
const ROOT_IDENTITIES = new WeakMap();
let nextRootIdentity = 1;

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
  45: ["a", "navigation-link"],
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

export function decodeApplicationTheme(input) {
  return decodeTheme(input, refuse);
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
    const hasValue = component === 15 || component === 16 || component === 17 || component === 22 || component === 31 || component === 32 || component === 38 || component === 39 || component === 43 || component === 44 || component === 45;
    if ((hasValue && (valueCapacity === 0 || valueCapacity > MAX_CONTROL_VALUE_BYTES || valueLength > valueCapacity))
      || (!hasValue && component !== 12 && (valueCapacity !== 0 || valueLength !== 0))) refuse("invalid-control-value");
    if (component === 12 && ((valueLength > 0 && (valueCapacity === 0 || valueCapacity > MAX_CONTROL_VALUE_BYTES || valueLength > valueCapacity)) || (valueLength === 0 && valueCapacity !== 0))) refuse("invalid-control-value");
    if ((index === 0 && parent !== null) || (index !== 0 && (parent === null || parent >= index))) refuse("unknown-parent");
    if (action !== null && action >= actions.length) refuse("unknown-action");
    if (component === 45 && action !== null) refuse("invalid-control-value");
    if ((component === 43 || component === 44) && action !== null && actions[action].event !== "change") refuse("invalid-control-value");
    const stateful = component === 8 || component === 15 || component === 16 || component === 17 || component === 43 || component === 44;
    if (state !== "ready" && (!stateful || action !== null)) refuse("invalid-node-state");
    const key = cursor.text(keyLength);
    const text = cursor.text(textLength);
    const value = cursor.text(valueLength);
    if ([12, 14, 39, 40, 41, 42, 43, 44, 45].includes(component) && text.length === 0) refuse("invalid-control-value");
    if ((component === 38 || component === 39) && !validProgress(value)) refuse("invalid-control-value");
    if (component === 38 && value.startsWith("0/")) refuse("invalid-control-value");
    if (keys.has(key)) refuse("duplicate-key");
    keys.add(key);
    const depth = parent === null ? 1 : depths[parent] + 1;
    if (depth > MAX_DEPTH) refuse("too-deep");
    depths.push(depth);
    return Object.freeze({ parent, component, state, action, key, text, value, valueCapacity });
  });
  for (const [index, node] of nodes.entries()) {
    if (node.component === 12 && node.value && !nodes.some((child, childIndex) => childIndex > index && child.parent === index && (child.component === 8 || child.component === 45) && child.key === node.value)) refuse("invalid-control-value");
    const children = nodes.filter((child) => child.parent === index);
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
      const kinds = new Set(labels.flatMap((label) => {
        const labelIndex = nodes.indexOf(label);
        return nodes.filter((child) => child.parent === labelIndex && (child.component === 43 || child.component === 44)).map((child) => child.component);
      }));
      if (children.filter((child) => child.component === 41).length !== 1 || labels.length === 0 || kinds.size !== 1) refuse("invalid-control-value");
    }
    if (node.component === 41 && (node.parent === null || nodes[node.parent].component !== 40)) refuse("invalid-control-value");
    if (node.component === 42 && (node.parent === null || nodes[node.parent].component !== 40 || children.filter((child) => child.component === 43 || child.component === 44).length !== 1)) refuse("invalid-control-value");
    if ((node.component === 43 || node.component === 44) && (!['true', 'false'].includes(node.value) || node.parent === null || nodes[node.parent].component !== 42)) refuse("invalid-control-value");
    if (node.component === 45 && (!['home', 'tour', 'creche', 'patchbay', 'source'].includes(node.value) || node.parent === null || nodes[node.parent].component !== 12)) refuse("invalid-control-value");
  }
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
    const state = NODE_STATE_IDENTITIES[node?.state ?? "ready"];
    if (!state) refuse("malformed-encoding");
    const parent = node.parent === null ? 255 : node.parent;
    const action = node.action === null ? 255 : node.action;
    if (!Number.isInteger(parent) || parent < 0 || parent > 255 || !Number.isInteger(action) || action < 0 || action > 255) refuse("malformed-encoding");
    const key = new TextEncoder().encode(node.key ?? "");
    const content = new TextEncoder().encode(node.text ?? "");
    const value = new TextEncoder().encode(node.value ?? "");
    const valueCapacity = node.valueCapacity ?? 0;
    if (key.length === 0 || key.length > MAX_KEY_BYTES || content.length > MAX_TEXT_BYTES) refuse("text-too-long");
    const hasValue = component === 15 || component === 16 || component === 17 || component === 22 || component === 31 || component === 32 || component === 38 || component === 39 || component === 43 || component === 44 || component === 45;
    if (!Number.isSafeInteger(valueCapacity) || valueCapacity < 0 || valueCapacity > MAX_CONTROL_VALUE_BYTES
      || (hasValue && (valueCapacity === 0 || value.length > valueCapacity))
      || (!hasValue && component !== 12 && (valueCapacity !== 0 || value.length !== 0))) refuse("invalid-control-value");
    if (component === 12 && ((value.length > 0 && (valueCapacity === 0 || valueCapacity > MAX_CONTROL_VALUE_BYTES || value.length > valueCapacity)) || (value.length === 0 && valueCapacity !== 0))) refuse("invalid-control-value");
    if ((component === 38 || component === 39) && !validProgress(new TextDecoder().decode(value))) refuse("invalid-control-value");
    if (component === 38 && new TextDecoder().decode(value).startsWith("0/")) refuse("invalid-control-value");
    const stateful = component === 8 || component === 15 || component === 16 || component === 17 || component === 43 || component === 44;
    if (state !== NODE_STATE_IDENTITIES.ready && (!stateful || action !== 255)) refuse("invalid-node-state");
    const nodeHeader = new Uint8Array(15);
    nodeHeader.set([parent, component, state, action, key.length, content.length & 0xff, content.length >>> 8]);
    const headerView = new DataView(nodeHeader.buffer);
    headerView.setUint32(7, value.length, true);
    headerView.setUint32(11, valueCapacity, true);
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
  if (event.currentTarget instanceof HTMLInputElement
    && (event.currentTarget.type === "checkbox" || event.currentTarget.type === "radio")) {
    return event.currentTarget.checked ? "true" : "false";
  }
  if (event.currentTarget instanceof HTMLInputElement || event.currentTarget instanceof HTMLTextAreaElement || event.currentTarget instanceof HTMLSelectElement) return event.currentTarget.value;
  return "";
}

const NAVIGATION_DESTINATIONS = Object.freeze({
  home: "/conduit",
  tour: "/conduit/tour/",
  creche: "/conduit/creche/",
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

function restoreInteraction(root, snapshot) {
  if (!snapshot) return;
  let target = Array.from(root.querySelectorAll("[data-application-key]"))
    .find((element) => element.dataset.applicationKey === snapshot.key && !element.disabled);
  if (!target) target = root.querySelector('[tabindex="0"]')
    ?? root.querySelector('button:not(:disabled), input:not(:disabled), select:not(:disabled), textarea:not(:disabled)');
  if (!(target instanceof HTMLElement)) {
    root.tabIndex = -1;
    target = root;
  }
  target.focus();
  if ((target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement) && snapshot.selection) {
    const end = target.value.length;
    target.setSelectionRange(Math.min(snapshot.selection.start, end), Math.min(snapshot.selection.end, end), snapshot.selection.direction);
  }
}

function installRovingFocus(container, selectedKey, selectedIndex) {
  const controls = Array.from(container.querySelectorAll("button:not(:disabled)"));
  if (controls.length === 0) return;
  let current = selectedKey ? controls.findIndex((control) => control.dataset.applicationKey === selectedKey) : selectedIndex;
  if (current < 0 || current >= controls.length) current = 0;
  controls.forEach((control, index) => { control.tabIndex = index === current ? 0 : -1; });
  container.addEventListener("keydown", (event) => {
    const focused = controls.indexOf(document.activeElement);
    if (focused < 0) return;
    let next = focused;
    if (event.key === "ArrowRight" || event.key === "ArrowDown") next = Math.min(focused + 1, controls.length - 1);
    else if (event.key === "ArrowLeft" || event.key === "ArrowUp") next = Math.max(focused - 1, 0);
    else if (event.key === "Home") next = 0;
    else if (event.key === "End") next = controls.length - 1;
    else return;
    event.preventDefault();
    controls.forEach((control, index) => { control.tabIndex = index === next ? 0 : -1; });
    controls[next].focus();
  });
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
  const active = root.contains(document.activeElement) ? document.activeElement : null;
  const activeNode = active?.closest?.("[data-application-key]");
  const interaction = activeNode ? {
    key: activeNode.dataset.applicationKey,
    selection: active instanceof HTMLInputElement || active instanceof HTMLTextAreaElement ? {
      start: active.selectionStart ?? 0, end: active.selectionEnd ?? 0, direction: active.selectionDirection ?? "none",
    } : null,
  } : null;
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
  const nodesByKey = new Map();
  const requestAction = (node, value = "") => {
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
    const encodedValue = typeof value === "string" ? new TextEncoder().encode(value) : value;
    if (!(encodedValue instanceof Uint8Array)) refuse("malformed-encoding");
    if (encodedValue.length > MAX_EVENT_BYTES) {
      lastRefusal = "event-too-large";
      options.onRefusal?.(lastRefusal);
      return null;
    }
    if (encodedValue.length > node.valueCapacity) {
      lastRefusal = "invalid-control-value";
      options.onRefusal?.(lastRefusal);
      return null;
    }
    const encodedBytes = EVENT_HEADER_BYTES + new TextEncoder().encode(action.id).length + encodedValue.length;
    if (queue.length === capacity || queuedBytes + encodedBytes > byteCapacity) {
      lastRefusal = "queue-pressure";
      options.onRefusal?.(lastRefusal);
      return null;
    }
    const pending = { revision: view.revision, action: action.id, kind: action.kind, value: encodedValue };
    const event = Object.freeze({ ...pending, encoded: encodeApplicationEvent(pending) });
    queue.push(event);
    queuedBytes += event.encoded.length;
    options.onEvent?.(event);
    return event;
  };
  for (const [nodeIndex, node] of view.nodes.entries()) {
    const [tag, kind] = COMPONENTS[node.component];
    const element = document.createElement(tag);
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
      if (node.value === "home") element.setAttribute("aria-label", "Conduit home");
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
    if (node.component === 15 || node.component === 16 || node.component === 17) {
      element.setAttribute("aria-label", node.text);
      element.value = node.value;
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
    if (node.component === 11) element.dataset.renderer = "patchbay";
    if (node.action !== null) {
      const action = view.actions[node.action];
      element.dataset.applicationAction = action.id;
      element.addEventListener(action.event, (browserEvent) => {
        requestAction(node, eventValue(browserEvent));
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
  root.replaceChildren(fragment);
  restoreInteraction(root, interaction);
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
    requestAction(key, value = "") {
      const node = nodesByKey.get(key);
      if (!node) refuse("unknown-component");
      return requestAction(node, value);
    },
  });
}

export function createApplicationPresentationHost(scope = document) {
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
      const encoded = description instanceof Uint8Array ? description : encodeApplicationView(description);
      const view = decodeApplicationView(encoded);
      const previous = revisions.get(slot) ?? 0;
      if (view.revision <= previous) refuse("stale-revision");
      const manifestation = manifestApplicationView(encoded, rootFor(slot), {
        eventCapacity: options.eventCapacity,
        eventByteCapacity: options.eventByteCapacity,
        choiceScope,
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

export const applicationPresentationLimits = Object.freeze({
  version: VERSION, retiredVersion: RETIRED_VERSION,
  themeVersion: applicationThemeLimits.version, retiredThemeVersion: applicationThemeLimits.retiredVersion,
  bytes: MAX_BYTES, nodes: MAX_NODES, depth: MAX_DEPTH, textBytes: MAX_TEXT_BYTES,
  actions: MAX_ACTIONS, resources: 0, controlValueBytes: MAX_CONTROL_VALUE_BYTES,
  eventBytes: MAX_EVENT_BYTES, eventQueue: MAX_EVENT_QUEUE, eventQueueBytes: MAX_EVENT_QUEUE_BYTES,
});
