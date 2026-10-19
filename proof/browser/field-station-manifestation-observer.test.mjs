import { test } from "node:test";
import assert from "node:assert/strict";
import { observeClockManifestation } from "./field-station/manifestation-observer.mjs";

test("first clock output survives a later frame before the proof reads the DOM", (context) => {
  const priorElement = globalThis.Element;
  const priorObserver = globalThis.MutationObserver;
  class Element {
    constructor(sequence) {
      this.dataset = sequence === undefined ? {} : { faceKind: "face/tick", observationSequence: String(sequence), faceId: `face/${sequence}` };
      this.textContent = `Clock tick: ${sequence}`;
      this.outputs = [];
    }
    matches() { return this.dataset.faceKind === "face/tick"; }
    querySelectorAll() { return this.outputs; }
  }
  let observer;
  class MutationObserver {
    constructor(callback) { this.callback = callback; this.pending = []; observer = this; }
    observe() {}
    takeRecords() { return this.pending.splice(0); }
    disconnect() { this.disconnected = true; }
  }
  globalThis.Element = Element;
  globalThis.MutationObserver = MutationObserver;
  context.after(() => { globalThis.Element = priorElement; globalThis.MutationObserver = priorObserver; });
  const root = new Element();
  const zero = new Element(0);
  const one = new Element(1);
  const retained = observeClockManifestation(root);
  root.outputs = [one];
  observer.pending.push(
    { type: "childList", removedNodes: [], addedNodes: [zero] },
    { type: "childList", removedNodes: [zero], addedNodes: [one] },
  );
  assert.equal(retained.first.text, "Clock tick: 0");
  assert.equal(retained.first.observationSequence, "0");
  assert.equal(retained.current.faceId, "face/1");
  zero.dataset.faceId = "changed";
  assert.equal(retained.first.faceId, "face/0");
  root.outputs = [];
  observer.pending.push({ type: "childList", removedNodes: [one], addedNodes: [] });
  assert.equal(retained.finish().faceId, "face/1");
  assert.equal(observer.disconnected, true);
});
