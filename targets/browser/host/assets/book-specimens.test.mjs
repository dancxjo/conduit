import assert from "node:assert/strict";
import test from "node:test";
import { conceptualTourStage, createTourStage, identifyTourSpecimen } from "./book-state.mjs";

const source = (name, message = "hello") => `form ${name} {\n  words: text/literal("${message}")\n}`;

test("canonical Form identity survives lesson insertion and reordering", () => {
  const a = identifyTourSpecimen(source("alpha"));
  identifyTourSpecimen(source("inserted"));
  assert.equal(identifyTourSpecimen(source("alpha")), a);
  assert.notEqual(identifyTourSpecimen(source("beta")), a);
});

test("two uses of one canonical Form share identity across presentation modes", () => {
  assert.equal(
    createTourStage(source("shared"), "two-host").identity,
    createTourStage(source("shared"), "two-host-plan").identity,
  );
});

test("lesson-local conceptual state is explicit and finite", () => {
  assert.deepEqual(conceptualTourStage("The Body"), {
    identity: "tour-concept:the-body", label: "The Body", mode: "conceptual",
  });
});

test("workspace geometry restores only admitted bounded presentation state", async () => {
  let written = null;
  const storage = {
    readJson: async (key) => key === "workspace-layout"
      ? { schema: "conduit.tour/workspace-layout@1", narrative_percent: 61 }
      : null,
    writeJson: async (key, value) => { written = { key, value }; },
  };
  const { openBookReadingState } = await import("./book-state.mjs");
  const state = await openBookReadingState(storage);
  assert.equal(state.workspace.narrativePercent, 61);
  await state.setNarrativePercent(35);
  assert.deepEqual(written, {
    key: "workspace-layout",
    value: { schema: "conduit.tour/workspace-layout@1", narrative_percent: 35 },
  });
  assert.throws(() => state.setNarrativePercent(66), /outside its admitted bound/);
});

test("malformed persisted workspace geometry refuses", async () => {
  const { openBookReadingState } = await import("./book-state.mjs");
  await assert.rejects(() => openBookReadingState({
    readJson: async (key) => key === "workspace-layout"
      ? { schema: "conduit.tour/workspace-layout@1", narrative_percent: 100 }
      : null,
    writeJson: async () => {},
  }), /workspace layout is malformed/);
});
