const KEYBOARD_IMPLEMENTATION = "browser/keyboard-events@1";
const POINTER_IMPLEMENTATION = "browser/pointer-events@1";
const MAXIMUM_QUEUE_ITEMS = 8;
const KEYBOARD_EVENT_BYTES = 3;
const KEYBOARD_QUEUE_BYTES = 24;
const POINTER_VALUE_BYTES = 65_536;

export class BrowserInputRefusal extends Error {
  constructor(code, message) {
    super(message);
    this.name = "BrowserInputRefusal";
    this.code = code;
  }
}

/**
 * Host-owned, finite adaptation of DOM input facts to portable Conduit values.
 * The caller supplies the current admitted Boot truth; Forms never see DOM
 * objects, selectors, key codes, focus state, or browser lifecycle facts.
 */
export function openBrowserHumanInput({
  target = globalThis.document,
  boot,
  currentBoot = () => boot,
  maximumQueueItems = MAXIMUM_QUEUE_ITEMS,
} = {}) {
  if (!target?.addEventListener || !target?.removeEventListener) {
    refuse("UnsupportedInput", "browser input requires an event target");
  }
  if (!Number.isInteger(maximumQueueItems) || maximumQueueItems < 1 || maximumQueueItems > MAXIMUM_QUEUE_ITEMS) {
    refuse("PressureBoundInvalid", "browser input queue bound is invalid");
  }
  const admitted = admittedImplementations(boot);
  const owner = Object.freeze({
    host_id: boundedIdentity(boot?.host_id ?? boot?.hostId, "HostIdentityInvalid"),
    boot_id: boundedIdentity(boot?.boot_id ?? boot?.bootId, "BootIdentityInvalid"),
    offer_generation: positiveInteger(boot?.offer_generation ?? 1, "OfferGenerationInvalid"),
  });
  const keyboardWaiters = [];
  const buttonWaiters = [];
  const pointerConsumers = new Set();
  let closed = false;
  let terminal = null;
  let pointerSequence = 0;
  let buttonSequence = 0;
  let dropped = 0;

  const onKey = (event) => {
    if (closed || keyboardWaiters.length === 0) return;
    const usage = browserKeyboardUsage(event.code);
    if (usage === null) return;
    const waiter = keyboardWaiters.shift();
    try {
      assertCurrent(owner, currentBoot());
      assertPageActive(target);
      if (!currentTargetOwnsEvent(target, event)) refuse("FocusLost", "keyboard focus left the admitted target");
      event.preventDefault();
      waiter.resolve(Object.freeze({
        schema: "input/key-event@1",
        canonical_bytes: Uint8Array.of(usage, event.type === "keydown" ? 0 : 1, modifiers(event)),
        owner,
      }));
    } catch (error) {
      waiter.reject(error);
    }
  };

  const onPointer = (event) => {
    if (closed || (pointerConsumers.size === 0 && buttonWaiters.length === 0)) return;
    try {
      assertCurrent(owner, currentBoot());
      assertPageActive(target);
      if (!currentTargetOwnsEvent(target, event)) refuse("FocusLost", "pointer target is no longer admitted");
      if (event.buttons !== 0 && event.buttons !== 1) {
        refuse("UnsupportedInput", "pointer buttons exceed the reviewed primary-button profile");
      }
      if (buttonWaiters.length > 0) {
        const waiter = buttonWaiters.shift();
        waiter.resolve(Object.freeze({
          schema: "input/button-transition@1",
          pressed: event.type === "pointerdown",
          sequence: buttonSequence++,
          owner,
        }));
      }
      const surface = target.nodeType === 9 ? target.documentElement : target;
      const bounds = surface.getBoundingClientRect();
      if (!(bounds.width > 0 && bounds.height > 0)) refuse("TargetLost", "pointer target has no extent");
      const millionth = (value) => Math.round(value * 1_000_000);
      const coalesced = typeof event.getCoalescedEvents === "function"
        ? Math.max(0, event.getCoalescedEvents().length - 1)
        : 0;
      const value = Object.freeze({
        schema: "input/pointer-event@1",
        position_x: millionth((event.clientX - bounds.left) / bounds.width),
        position_y: millionth((event.clientY - bounds.top) / bounds.height),
        delta_x: millionth(event.movementX / bounds.width),
        delta_y: millionth(event.movementY / bounds.height),
        primary_pressed: event.buttons === 1,
        coalesced,
        dropped,
        queue_capacity: 1,
        sequence: pointerSequence,
        owner,
      });
      pointerSequence += 1;
      dropped = 0;
      for (const consume of pointerConsumers) consume(value);
    } catch (error) {
      while (buttonWaiters.length) buttonWaiters.shift().reject(error);
      for (const consume of pointerConsumers) consume(null, error);
    }
  };

  const end = (code) => {
    if (closed) return;
    terminal = new BrowserInputRefusal(code, code === "PageLost"
      ? "browser page was lost while input was pending"
      : "browser input target lost focus");
    while (keyboardWaiters.length) keyboardWaiters.shift().reject(terminal);
    while (buttonWaiters.length) buttonWaiters.shift().reject(terminal);
  };
  const restoreFocus = () => {
    if (terminal?.code === "FocusLost") terminal = null;
  };
  const document = target.nodeType === 9 ? target : target.ownerDocument;
  const window = document?.defaultView ?? globalThis;
  target.addEventListener("keydown", onKey, true);
  target.addEventListener("keyup", onKey, true);
  target.addEventListener("pointerdown", onPointer, true);
  target.addEventListener("pointerup", onPointer, true);
  const pageLost = () => end("PageLost");
  const focusLost = () => end("FocusLost");
  window.addEventListener?.("pagehide", pageLost, { once: true });
  window.addEventListener?.("blur", focusLost);
  window.addEventListener?.("focus", restoreFocus);

  return Object.freeze({
    schema: "conduit.browser/human-input-adapter@1",
    owner,
    limits: Object.freeze({
      keyboard_queue_items: maximumQueueItems,
      keyboard_event_bytes: KEYBOARD_EVENT_BYTES,
      keyboard_queue_bytes: KEYBOARD_QUEUE_BYTES,
      pointer_queue_items: 1,
      pointer_value_bytes: POINTER_VALUE_BYTES,
    }),
    nextKeyboard() {
      requireSelected(admitted, KEYBOARD_IMPLEMENTATION);
      if (closed || terminal) return Promise.reject(terminal ?? new BrowserInputRefusal("Cancelled", "browser input adapter is closed"));
      if (keyboardWaiters.length >= maximumQueueItems) {
        return Promise.reject(new BrowserInputRefusal("Pressure", "keyboard input queue is full"));
      }
      return new Promise((resolve, reject) => keyboardWaiters.push({ resolve, reject }));
    },
    nextButton() {
      requireSelected(admitted, POINTER_IMPLEMENTATION);
      if (closed || terminal) return Promise.reject(terminal ?? new BrowserInputRefusal("Cancelled", "browser input adapter is closed"));
      if (buttonWaiters.length >= maximumQueueItems) {
        return Promise.reject(new BrowserInputRefusal("Pressure", "button input queue is full"));
      }
      return new Promise((resolve, reject) => buttonWaiters.push({ resolve, reject }));
    },
    observePointer(consume) {
      requireSelected(admitted, POINTER_IMPLEMENTATION);
      if (typeof consume !== "function") throw new TypeError("pointer consumer must be a function");
      if (closed || terminal) refuse(terminal?.code ?? "Cancelled", "browser input adapter is unavailable");
      if (pointerConsumers.size >= maximumQueueItems) refuse("Pressure", "pointer consumer bound is full");
      pointerConsumers.add(consume);
      return () => pointerConsumers.delete(consume);
    },
    cancelPending() {
      while (keyboardWaiters.length) keyboardWaiters.shift().reject(new BrowserInputRefusal("Cancelled", "browser input request was cancelled"));
      while (buttonWaiters.length) buttonWaiters.shift().reject(new BrowserInputRefusal("Cancelled", "browser input request was cancelled"));
    },
    close() {
      if (closed) return;
      closed = true;
      target.removeEventListener("keydown", onKey, true);
      target.removeEventListener("keyup", onKey, true);
      target.removeEventListener("pointerdown", onPointer, true);
      target.removeEventListener("pointerup", onPointer, true);
      window.removeEventListener?.("pagehide", pageLost);
      window.removeEventListener?.("blur", focusLost);
      window.removeEventListener?.("focus", restoreFocus);
      while (keyboardWaiters.length) keyboardWaiters.shift().reject(new BrowserInputRefusal("Cancelled", "browser input request was cancelled"));
      while (buttonWaiters.length) buttonWaiters.shift().reject(new BrowserInputRefusal("Cancelled", "browser input request was cancelled"));
      pointerConsumers.clear();
    },
  });
}

function admittedImplementations(boot) {
  const entries = boot?.implementation_registry ?? boot?.implementations;
  if (!Array.isArray(entries) || entries.length > 64) refuse("BootTruthInvalid", "browser Boot implementation registry is missing or unbounded");
  return new Set(entries.map((entry) => typeof entry === "string" ? entry : entry?.id));
}
function requireSelected(admitted, identity) {
  if (!admitted.has(identity)) refuse("UnsupportedInput", `${identity} was not selected in this browser PROFILE`);
}
function assertCurrent(owner, current) {
  if ((current?.boot_id ?? current?.bootId) !== owner.boot_id) refuse("StaleBoot", "input belongs to a stale browser Boot");
  if ((current?.offer_generation ?? 1) !== owner.offer_generation) refuse("StaleOffer", "input belongs to a stale browser offer generation");
}
function assertPageActive(target) {
  const document = target.nodeType === 9 ? target : target.ownerDocument;
  if (document?.visibilityState && document.visibilityState !== "visible") refuse("PageInactive", "browser page is not active");
}
function currentTargetOwnsEvent(target, event) {
  if (target.nodeType === 9) return true;
  return target === event.target || target.contains?.(event.target);
}
function modifiers(event) {
  return (event.ctrlKey ? 1 : 0) | (event.shiftKey ? 2 : 0) | (event.altKey ? 4 : 0) | (event.metaKey ? 8 : 0);
}
function browserKeyboardUsage(code) {
  if (/^Key[A-Z]$/.test(code)) return 0x04 + code.charCodeAt(3) - 65;
  if (/^Digit[1-9]$/.test(code)) return 0x1e + Number(code.slice(5)) - 1;
  if (code === "Digit0") return 0x27;
  return ({ Enter: 0x28, Escape: 0x29, Backspace: 0x2a, Tab: 0x2b, Space: 0x2c })[code] ?? null;
}
function boundedIdentity(value, code) {
  if (typeof value !== "string" || value.length < 1 || value.length > 256) refuse(code, "browser input owner identity is invalid");
  return value;
}
function positiveInteger(value, code) {
  if (!Number.isSafeInteger(value) || value < 1) refuse(code, "browser input generation is invalid");
  return value;
}
function refuse(code, message) { throw new BrowserInputRefusal(code, message); }
