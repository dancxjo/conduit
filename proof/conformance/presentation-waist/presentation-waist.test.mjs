import assert from "node:assert/strict";
import test from "node:test";
import { accountGraphical, accountLinear, accountSpoken, graphicalProject, linearProject, semanticAccount, specimens, spokenProject } from "./specimens.mjs";

const required = ["authored-argument", "named-collections", "source-destination", "spreadsheet", "timeline-editor", "web-article", "map", "terminal", "screen-reader", "patchbay"];
const forbiddenFaceKeys = new Set(["x", "y", "width", "height", "pane", "column", "font", "color", "pixels", "animation", "schedule", "iterator", "stateMachine"]);

// These requirements are written from the human account, independently of the
// Face records below. A projector cannot pass merely by round-tripping whatever
// graph it received: each named understanding must retain the exact semantic
// facts that make that understanding warranted.
const humanMeaningRequirements = {
  "authored-argument": {
    claim: ["subject:claim"],
    "evidence supports claim": ["relationship:evidence argument/supports claim"],
    "qualification follows evidence": ["relationship:qualification argument/qualifies claim", "composition:qualification RevealAfter evidence"],
  },
  "named-collections": {
    "Tools contains Editor and Mail": ["relationship:tools Contains editor", "relationship:tools Contains mail"],
    "Editor is focused": ["relationship:tools encounter/focuses editor"],
  },
  "source-destination": {
    "Archive is source": ["subject:source"],
    "Backup is destination": ["subject:destination", "relationship:report file/copy-destination destination"],
    "report.txt is selected": ["subject:report", "composition:report Group source"],
  },
  spreadsheet: {
    "B2 and C2 contribute to Total": ["relationship:b2 calculation/input-to total", "relationship:c2 calculation/input-to total"],
    "cell addresses are semantic identity": ["subject:b2", "subject:c2", "subject:total"],
  },
  "timeline-editor": {
    "Voice and Music coexist": ["composition:voice Juxtapose music"],
    "Verse take is selected on Voice": ["relationship:voice Contains clip", "composition:clip Emphasize voice"],
  },
  "web-article": {
    "aside qualifies section": ["relationship:section Contains aside", "composition:aside Subordinate section"],
    "reference belongs with section": ["relationship:section Describes reference", "composition:reference Associate section"],
  },
  map: {
    "route goes from station to museum": ["relationship:route route/from station", "relationship:route route/to museum"],
  },
  terminal: {
    "Tests passed is result of cargo test": ["relationship:result terminal/result-of command", "composition:result RevealAfter command"],
  },
  "screen-reader": {
    "MFA is enabled": ["subject:mfa"],
    "MFA status is focused in Security": ["relationship:security Contains mfa", "relationship:document encounter/focuses mfa", "composition:mfa Emphasize security"],
  },
  patchbay: {
    "Text flow connects input to output": ["relationship:cord cord/from input", "relationship:cord cord/to output"],
    "direction is input to output": ["subject:input", "subject:output", "relationship:cord cord/from input", "relationship:cord cord/to output"],
  },
};

function retainedFacts(account) {
  return new Set([
    ...account.subjects.map(value => `subject:${value}`),
    ...account.relationships.map(value => `relationship:${value}`),
    ...account.composition.map(value => `composition:${value}`),
    ...account.actions.map(value => `action:${value}`),
  ]);
}

function assertIndependentHumanAccount(specimen, account) {
  const requirements = humanMeaningRequirements[specimen.identity];
  assert.deepEqual(Object.keys(requirements), specimen.expected.understands);
  const facts = retainedFacts(account);
  for (const [meaning, required] of Object.entries(requirements)) {
    for (const fact of required) assert.ok(facts.has(fact), `${meaning} lost ${fact}`);
  }
  assert.deepEqual(account.actions, specimen.expected.can);
}

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

  test(`${specimen.identity}: three materially different projectors preserve one semantic account`, () => {
    const expected = semanticAccount(specimen);
    const shows = [graphicalProject(specimen), spokenProject(specimen), linearProject(specimen)];
    assert.deepEqual(shows.map(({ medium }) => medium), ["graphical", "spoken", "deterministic-linear"]);
    assert.deepEqual(accountGraphical(shows[0]), expected);
    assert.deepEqual(accountSpoken(shows[1]), expected);
    assert.deepEqual(accountLinear(shows[2]), expected);
    assertIndependentHumanAccount(specimen, accountGraphical(shows[0]));
    assertIndependentHumanAccount(specimen, accountSpoken(shows[1]));
    assertIndependentHumanAccount(specimen, accountLinear(shows[2]));
    assert.notEqual(shows[0].technique, shows[1].technique);
    assert.notEqual(shows[1].technique, shows[2].technique);
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

test("lost appearance is harmless while semantic loss is detected", () => {
  const specimen = specimens.find(({ identity }) => identity === "source-destination");
  const graphical = graphicalProject(specimen);
  const spoken = spokenProject(specimen);
  assert.notEqual(graphical.technique, spoken.technique);
  assert.deepEqual(accountGraphical(graphical), accountSpoken(spoken));

  const lossy = structuredClone(spoken);
  lossy.utterances = lossy.utterances.filter(statement => !statement.includes("file/copy-destination"));
  assert.notDeepEqual(accountSpoken(lossy), accountGraphical(graphical));
  assert.throws(
    () => assertIndependentHumanAccount(specimen, accountSpoken(lossy)),
    /Backup is destination lost relationship:report file\/copy-destination destination/,
  );
});

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
