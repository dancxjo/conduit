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
 * host-owned, finite adaptation of DOM input facts to portable Conduit values.
 * The caller supplies the current admitted Boot truth; Plots never see DOM
 * objects, selectors, key codes, focus state, or browser lifecycle facts.
 */
export function openBrowserHumanInput({
  target = globalThis.document,
  boot,
  currentBoot = () => boot,
  maximumQueueItems = MAXIMUM_QUEUE_ITEMS,
  routeInput,
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
  const keyboardQueue = [];
  const buttonWaiters = [];
  const buttonQueue = [];
  let buttonActive = false;
  const pointerConsumers = new Set();
  let closed = false;
  let terminal = null;
  let keyboardFailure = null;
  let buttonFailure = null;
  let pointerSequence = 0;
  let buttonSequence = 0;
  let dropped = 0;
  const pressedKeys = new Uint8Array(256);
  const routedKeys = new Uint8Array(256);
  let modifierBits = 0;

  const onKey = (event) => {
    if (closed || keyboardFailure) return;
    const usage = browserKeyboardUsage(event.code);
    if (usage === null) return;
    // Track physical state even while a native control owns the interaction.
    // Aggregate DOM modifier flags cannot identify left versus right sides.
    const pressed = event.type === "keydown";
    if (event.repeat || Boolean(pressedKeys[usage]) === pressed) return;
    pressedKeys[usage] = Number(pressed);
    if (usage >= 0xe0) {
      const bit = 1 << (usage - 0xe0);
      modifierBits = pressed ? modifierBits | bit : modifierBits & ~bit;
    }
    const nativeControl = nativeKeyboardControl(event);
    const wasRouted = Boolean(routedKeys[usage]);
    // A standing Plot request does not own a native select, button, link or
    // text editor. Only a release of an earlier Plot press crosses that boundary;
    // it balances the transition without cancelling the native default action.
    if (nativeControl && (pressed || !wasRouted)) return;
    if (!pressed && !wasRouted) return;
    if (!pressed) routedKeys[usage] = 0;
    try {
      assertCurrent(owner, currentBoot());
      assertPageActive(target);
      if (!currentTargetOwnsEvent(target, event)) refuse("FocusLost", "keyboard focus left the admitted target");
      const canonical = Uint8Array.of(usage, pressed ? 0 : 1, modifierBits);
      const deliveryPlot = routeInput?.("keyboard", canonical);
      if (routeInput && deliveryPlot === null) return;
      if (pressed) routedKeys[usage] = 1;
      if (!nativeControl) event.preventDefault();
      const transition = Object.freeze({
        schema: "input/key-event@1", canonical_bytes: canonical, owner,
        ...(routeInput ? { delivery_plot: deliveryPlot } : {}),
      });
      if (keyboardWaiters.length > 0) keyboardWaiters.shift().resolve(transition);
      else {
        if (keyboardQueue.length >= maximumQueueItems) {
          keyboardFailure = new BrowserInputRefusal("Pressure", "ordered keyboard queue is full");
          keyboardQueue.length = 0;
          while (keyboardWaiters.length) keyboardWaiters.shift().reject(keyboardFailure);
          return;
        }
        keyboardQueue.push(transition);
      }
    } catch (error) {
      keyboardFailure = error;
      keyboardQueue.length = 0;
      while (keyboardWaiters.length) keyboardWaiters.shift().reject(error);
    }
  };

  const onPointer = (event) => {
    if (closed || (pointerConsumers.size === 0 && !buttonActive)) return;
    try {
      if (event.type === "pointerdown" && target.nodeType !== 9 && typeof target.setPointerCapture === "function") {
        target.setPointerCapture(event.pointerId);
      }
      assertCurrent(owner, currentBoot());
      assertPageActive(target);
      if (!currentTargetOwnsEvent(target, event)) refuse("FocusLost", "pointer target is no longer admitted");
      if (event.buttons !== 0 && event.buttons !== 1) {
        refuse("UnsupportedInput", "pointer buttons exceed the reviewed primary-button profile");
      }
      const isButtonTransition = event.type === "pointerdown" || event.type === "pointerup";
      const buttonPlot = buttonActive && isButtonTransition
        ? routeInput?.("button", { pressed: event.type === "pointerdown" })
        : null;
      if (buttonActive && isButtonTransition && !buttonFailure && (!routeInput || buttonPlot !== null)) {
        const transition = Object.freeze({
          schema: "input/button-transition@1",
          pressed: event.type === "pointerdown",
          ...(routeInput ? { delivery_plot: buttonPlot } : {}),
          sequence: buttonSequence++,
          owner,
        });
        if (buttonWaiters.length > 0) buttonWaiters.shift().resolve(transition);
        else {
          if (buttonQueue.length >= maximumQueueItems) {
            buttonFailure = new BrowserInputRefusal("Pressure", "ordered button queue is full");
            buttonQueue.length = 0;
            while (buttonWaiters.length) buttonWaiters.shift().reject(buttonFailure);
          } else buttonQueue.push(transition);
        }
      }
      if (pointerConsumers.size === 0) return;
      const pointerPlot = routeInput?.("pointer", null);
      if (routeInput && pointerPlot === null) return;
      const surface = target.nodeType === 9 ? target.documentElement : target;
      const bounds = surface.getBoundingClientRect();
      if (!(bounds.width > 0 && bounds.height > 0)) refuse("TargetLost", "pointer target has no extent");
      const boundedAxis = (value) => Math.max(-1_000_000, Math.min(1_000_000, value));
      const boundedPosition = (value) => Math.max(0, Math.min(1_000_000, value));
      const millionth = (value) => Math.round(value * 1_000_000);
      const coalesced = typeof event.getCoalescedEvents === "function"
        ? Math.max(0, event.getCoalescedEvents().length - 1)
        : 0;
      const value = Object.freeze({
        schema: "input/pointer-event@1",
        ...(routeInput ? { delivery_plot: pointerPlot } : {}),
        position_x: boundedPosition(millionth((event.clientX - bounds.left) / bounds.width)),
        position_y: boundedPosition(millionth((event.clientY - bounds.top) / bounds.height)),
        delta_x: boundedAxis(millionth(event.movementX / bounds.width)),
        delta_y: boundedAxis(millionth(event.movementY / bounds.height)),
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
      terminal = error;
      buttonQueue.length = 0;
      keyboardQueue.length = 0;
      while (keyboardWaiters.length) keyboardWaiters.shift().reject(error);
      while (buttonWaiters.length) buttonWaiters.shift().reject(error);
      for (const consume of pointerConsumers) consume(null, error);
    }
  };

  const end = (code) => {
    if (closed) return;
    terminal = new BrowserInputRefusal(code, code === "PageLost"
      ? "browser page was lost while input was pending"
      : "browser input target lost focus");
    buttonQueue.length = 0;
    keyboardQueue.length = 0;
    pressedKeys.fill(0);
    routedKeys.fill(0);
    modifierBits = 0;
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
  target.addEventListener("pointermove", onPointer, true);
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
      button_queue_items: maximumQueueItems,
      pointer_queue_items: 1,
      pointer_value_bytes: POINTER_VALUE_BYTES,
    }),
    nextKeyboard() {
      requireSelected(admitted, KEYBOARD_IMPLEMENTATION);
      if (closed || terminal || keyboardFailure) return Promise.reject(terminal ?? keyboardFailure ?? new BrowserInputRefusal("Cancelled", "browser input adapter is closed"));
      try {
        assertCurrent(owner, currentBoot());
        assertPageActive(target);
      } catch (error) {
        terminal = error;
        keyboardQueue.length = 0;
        return Promise.reject(error);
      }
      if (keyboardQueue.length) return Promise.resolve(keyboardQueue.shift());
      if (keyboardWaiters.length >= maximumQueueItems) {
        return Promise.reject(new BrowserInputRefusal("Pressure", "keyboard input queue is full"));
      }
      return new Promise((resolve, reject) => keyboardWaiters.push({ resolve, reject }));
    },
    nextButton() {
      requireSelected(admitted, POINTER_IMPLEMENTATION);
      if (closed || terminal || buttonFailure) return Promise.reject(terminal ?? buttonFailure ?? new BrowserInputRefusal("Cancelled", "browser input adapter is closed"));
      try {
        assertCurrent(owner, currentBoot());
        assertPageActive(target);
      } catch (error) {
        terminal = error;
        buttonQueue.length = 0;
        return Promise.reject(error);
      }
      buttonActive = true;
      if (buttonQueue.length) return Promise.resolve(buttonQueue.shift());
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
      buttonActive = false;
      keyboardQueue.length = 0;
      buttonQueue.length = 0;
      while (keyboardWaiters.length) keyboardWaiters.shift().reject(new BrowserInputRefusal("Cancelled", "browser input request was cancelled"));
      while (buttonWaiters.length) buttonWaiters.shift().reject(new BrowserInputRefusal("Cancelled", "browser input request was cancelled"));
    },
    close() {
      if (closed) return;
      closed = true;
      target.removeEventListener("keydown", onKey, true);
      target.removeEventListener("keyup", onKey, true);
      target.removeEventListener("pointerdown", onPointer, true);
      target.removeEventListener("pointermove", onPointer, true);
      target.removeEventListener("pointerup", onPointer, true);
      window.removeEventListener?.("pagehide", pageLost);
      window.removeEventListener?.("blur", focusLost);
      window.removeEventListener?.("focus", restoreFocus);
      keyboardQueue.length = 0;
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
function nativeKeyboardControl(event) {
  const selector = "a[href], button, input, select, textarea, summary, [contenteditable]:not([contenteditable='false'])";
  const path = typeof event.composedPath === "function" ? event.composedPath() : [event.target];
  return path.some((node) => typeof node?.closest === "function" && node.closest(selector));
}

// DOM code names identify physical positions; event.key contains the host
// layout's interpretation and must never choose Conduitese text or dead keys.
function browserKeyboardUsage(code) {
  if (/^Key[A-Z]$/.test(code)) return 0x04 + code.charCodeAt(3) - 65;
  if (/^Digit[1-9]$/.test(code)) return 0x1e + Number(code.slice(5)) - 1;
  if (/^F([1-9]|1[0-9]|2[0-4])$/.test(code)) {
    const number = Number(code.slice(1));
    return number <= 12 ? 0x39 + number : 0x67 + number - 12;
  }
  return KEYBOARD_USAGES[code] ?? null;
}
const KEYBOARD_USAGES = Object.freeze({
  Digit0: 0x27, Enter: 0x28, Escape: 0x29, Backspace: 0x2a,
  Tab: 0x2b, Space: 0x2c, Minus: 0x2d, Equal: 0x2e,
  BracketLeft: 0x2f, BracketRight: 0x30, Backslash: 0x31,
  Semicolon: 0x33, Quote: 0x34, Backquote: 0x35, Comma: 0x36,
  Period: 0x37, Slash: 0x38, CapsLock: 0x39, PrintScreen: 0x46,
  ScrollLock: 0x47, Pause: 0x48, Insert: 0x49, Home: 0x4a,
  PageUp: 0x4b, Delete: 0x4c, End: 0x4d, PageDown: 0x4e,
  ArrowRight: 0x4f, ArrowLeft: 0x50, ArrowDown: 0x51, ArrowUp: 0x52,
  NumLock: 0x53, NumpadDivide: 0x54, NumpadMultiply: 0x55,
  NumpadSubtract: 0x56, NumpadAdd: 0x57, NumpadEnter: 0x58,
  Numpad1: 0x59, Numpad2: 0x5a, Numpad3: 0x5b, Numpad4: 0x5c,
  Numpad5: 0x5d, Numpad6: 0x5e, Numpad7: 0x5f, Numpad8: 0x60,
  Numpad9: 0x61, Numpad0: 0x62, NumpadDecimal: 0x63,
  IntlBackslash: 0x64, ContextMenu: 0x65, Power: 0x66, NumpadEqual: 0x67,
  Open: 0x74, Help: 0x75, Select: 0x77, Again: 0x79, Undo: 0x7a,
  Cut: 0x7b, Copy: 0x7c, Paste: 0x7d, Find: 0x7e,
  AudioVolumeMute: 0x7f, AudioVolumeUp: 0x80, AudioVolumeDown: 0x81,
  NumpadComma: 0x85, IntlRo: 0x87, KanaMode: 0x88, IntlYen: 0x89,
  Convert: 0x8a, NonConvert: 0x8b, Lang1: 0x90, Lang2: 0x91,
  Lang3: 0x92, Lang4: 0x93, Lang5: 0x94,
  ControlLeft: 0xe0, ShiftLeft: 0xe1, AltLeft: 0xe2, MetaLeft: 0xe3,
  ControlRight: 0xe4, ShiftRight: 0xe5, AltRight: 0xe6, MetaRight: 0xe7,
});
function boundedIdentity(value, code) {
  if (typeof value !== "string" || value.length < 1 || value.length > 256) refuse(code, "browser input owner identity is invalid");
  return value;
}
function positiveInteger(value, code) {
  if (!Number.isSafeInteger(value) || value < 1) refuse(code, "browser input generation is invalid");
  return value;
}
function refuse(code, message) { throw new BrowserInputRefusal(code, message); }
