import assert from "node:assert/strict";
import test from "node:test";
import { conceptualTourStage, createTourStage, identifyTourSpecimen } from "./tour-state.mjs";
import { parseTourPages } from "./tour-routing.mjs";

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
  assert.deepEqual(conceptualTourStage("The Body", "body-continuity"), {
    identity: "tour-companion:body-continuity", label: "The Body", mode: "conceptual",
  });
});

test("Tour page metadata owns stable route, companion, and stage topology", () => {
  const source = `---
page: first-form
route: a-form
companion: form-laboratory
stage: canonical-form:hello|run
---
# A Form

\`\`\`conduit run
form hello {}
\`\`\``;
  assert.deepEqual(parseTourPages([source])[0], {
    identity: "first-form", route: "a-form", companion: "form-laboratory", title: "A Form",
    stages: [{ identity: "canonical-form:hello", mode: "run" }],
    markdown: "# A Form\n\n```conduit run\nform hello {}\n```",
  });
  assert.throws(() => parseTourPages(["# Accidental topology"]), /metadata is missing/);
  assert.throws(() => parseTourPages([source, source]), /duplicated/);
});

test("workspace geometry restores only admitted bounded presentation state", async () => {
  let written = null;
  const storage = {
    readJson: async (key) => key === "workspace-layout"
      ? { schema: "conduit.tour/workspace-layout@1", narrative_percent: 61 }
      : null,
    writeJson: async (key, value) => { written = { key, value }; },
  };
  const { openTourReadingState } = await import("./tour-state.mjs");
  const state = await openTourReadingState(storage);
  assert.equal(state.workspace.narrativePercent, 61);
  assert.equal(state.workspace.patchbayPercent, 55);
  assert.equal(state.workspace.sourcePercent, 60);
  await state.setNarrativePercent(35);
  assert.deepEqual(written, {
    key: "workspace-layout",
    value: {
      schema: "conduit.tour/workspace-layout@2",
      narrative_percent: 35,
      patchbay_percent: 55,
      source_percent: 60,
    },
  });
  await state.setPatchbayPercent(70);
  await state.setSourcePercent(40);
  assert.deepEqual(state.workspace, { narrativePercent: 35, patchbayPercent: 70, sourcePercent: 40 });
  assert.throws(() => state.setNarrativePercent(66), /outside its admitted bound/);
  assert.throws(() => state.setPatchbayPercent(71), /outside its admitted bound/);
  assert.throws(() => state.setSourcePercent(39), /outside its admitted bound/);
});

test("current workspace geometry restores all independent pane preferences", async () => {
  const { openTourReadingState } = await import("./tour-state.mjs");
  const state = await openTourReadingState({
    readJson: async (key) => key === "workspace-layout" ? {
      schema: "conduit.tour/workspace-layout@2",
      narrative_percent: 42,
      patchbay_percent: 68,
      source_percent: 44,
    } : null,
    writeJson: async () => {},
  });
  assert.deepEqual(state.workspace, { narrativePercent: 42, patchbayPercent: 68, sourcePercent: 44 });
});

test("malformed persisted workspace geometry refuses", async () => {
  const { openTourReadingState } = await import("./tour-state.mjs");
  await assert.rejects(() => openTourReadingState({
    readJson: async (key) => key === "workspace-layout"
      ? { schema: "conduit.tour/workspace-layout@1", narrative_percent: 100 }
      : null,
    writeJson: async () => {},
  }), /workspace layout is malformed/);
});
