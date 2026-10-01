import assert from "node:assert/strict";
import test from "node:test";
import { specimens } from "./specimens.mjs";

const required = ["authored-argument", "named-collections", "source-destination", "spreadsheet", "timeline-editor", "web-article", "map", "terminal", "screen-reader", "patchbay"];
const forbiddenFaceKeys = new Set(["x", "y", "width", "height", "pane", "column", "font", "color", "pixels", "animation", "schedule", "iterator", "stateMachine"]);

test("the bounded matrix contains every adversarial specimen", () => {
  assert.deepEqual(specimens.map(({ identity }) => identity), required);
  assert.equal(new Set(required).size, specimens.length);
});

for (const specimen of specimens) {
  test(`${specimen.identity}: ownership remains explicit and realization-independent`, () => {
    assert.deepEqual(Object.keys(specimen.ownership), ["truth", "selection", "encounter", "realization", "show"]);
    assert.deepEqual(Object.keys(specimen.ownership.realization), ["graphical", "spoken", "linear"]);
    assert.ok(specimen.ownership.truth.includes("authoritative-domain-state"));
    assert.ok(specimen.ownership.selection.includes("current-relevance"));
    assert.ok(specimen.ownership.encounter.endsWith("/presentation"));
    assert.ok(specimen.ownership.show.endsWith("/finite-occurrence"));
  });

  test(`${specimen.identity}: actions remain descriptions rather than execution`, () => {
    for (const offered of specimen.presentation.actions) {
      assert.equal(offered.availability, "available");
      assert.ok(offered.intent.startsWith("encounter/"));
      assert.equal("execute" in offered, false);
      assert.equal("handler" in offered, false);
    }
  });

  test(`${specimen.identity}: Face contains no medium furniture or application machine`, () => {
    const visit = value => {
      if (!value || typeof value !== "object") return;
      for (const [key, child] of Object.entries(value)) {
        assert.equal(forbiddenFaceKeys.has(key), false, `forbidden Face key ${key}`);
        visit(child);
      }
    };
    visit(specimen.presentation);
  });
}

test("Patchbay is one ordinary specimen, not a grammar owner", () => {
  const patchbay = specimens.find(({ identity }) => identity === "patchbay");
  assert.ok(patchbay);
  assert.equal(Object.hasOwn(patchbay.presentation, "patchbay"), false);
  assert.equal(Object.hasOwn(patchbay.presentation, "canvas"), false);
});

test("the current matrix proposes no additional universal primitive", () => {
  const admittedKinds = new Set(["Group", "Contrast", "Juxtapose", "Emphasize", "Subordinate", "Associate", "RevealAfter"]);
  for (const specimen of specimens) {
    for (const relation of specimen.presentation.composition) assert.ok(admittedKinds.has(relation.kind));
  }
});
