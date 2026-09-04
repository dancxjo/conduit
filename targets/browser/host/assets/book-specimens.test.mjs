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
