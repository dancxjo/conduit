import assert from "node:assert/strict";
import test from "node:test";
import { setImmediate as nextTurn } from "node:timers/promises";
import {
  createPhysicalHostTargetCatalog,
  PHYSICAL_HOST_EVIDENCE_MAXIMA,
} from "../../products/creche/browser/creche-target-catalog.mjs";
import { createPhysicalHostRunner } from "../../products/creche/browser/creche-physical.mjs";

// Deterministic contract proof through the live catalog, workflow, and physical
// presentation helpers. These test-only document, presentation, and current-Body
// ABI fixtures prove neither DOM/browser acceptance nor Body/kernel execution.
// Run through cargo xtask check browser; no retired product shell is launched.

function targetContribution(suffix = "one", methods = {}, suppliedBounds = {}) {
  const target = {
    id: `fixture/target-${suffix}`,
    label: `Fixture ${suffix}`,
    model_id: `fixture/model-${suffix}`,
    profile_id: `fixture/profile-${suffix}`,
  };
  const modes = [
    { id: "fabricate-new", resultKind: "artifact", supported: true },
    { id: "install-existing", resultKind: "installation", supported: false },
    { id: "attach-running", resultKind: "attachment", supported: false },
  ];
  const bounds = {
    maximumOperations: 5,
    maximumOperationEvidenceBytes: 1024,
    maximumRetainedEvidenceBytes: 4096,
    ...suppliedBounds,
  };
  const adapter = {
    schema: "conduit.creche/physical-host-target-adapter@1",
    target,
    modes,
    bounds,
    createOptions() { return null; },
    async obtain() {},
    async bind() {},
    async realize() {},
    async observe() {},
    async cancel() {},
    ...methods,
  };
  return {
    schema: "conduit.creche/physical-host-target-entry@1",
    family: { id: "fixture/family", label: "Fixture family" },
    target,
    intentions: modes,
    fabrication_strategies: [{ id: "fixture/strategy", label: "Fixture strategy" }],
    carriers: { deployment: [], installation: [], attachment: [], observation: [] },
    bounds,
    expected_join_contract: "fixture/join@1",
    target_profile: { schema: "fixture/target-profile@1" },
    createAdapter: () => adapter,
  };
}

function runnerFixture(contribution, callbacks = {}) {
  const views = new Map();
  const options = { replaceChildren() {} };
  const section = {
    querySelector(selector) {
      assert.equal(selector, ".target-options");
      return options;
    },
    querySelectorAll(selector) {
      assert.equal(selector, ".target-options button, .target-options input, .target-options select");
      return [];
    },
  };

  // Only the existing current-Body output ABI is supplied. There is no WASM
  // instance, admission implementation, or substitute runtime in this fixture.
  const encoded = new TextEncoder().encode(JSON.stringify({ body_id: "body/delegated" }));
  const runtime = {
    memory: { buffer: encoded.buffer },
    conduit_creche_current: () => 0,
    conduit_creche_output_ptr: () => 0,
    conduit_creche_output_len: () => encoded.length,
    conduit_creche_input_capacity() { assert.fail("ambient Body admission must not run"); },
    conduit_creche_admit_physical_spore() { assert.fail("ambient Body admission must not run"); },
  };

  // The document seam is synchronous construction only. Restore the exact
  // descriptor before any asynchronous operation completes; do not run these
  // global-fixture cases concurrently.
  const savedDocument = Object.getOwnPropertyDescriptor(globalThis, "document");
  Object.defineProperty(globalThis, "document", {
    configurable: true,
    value: {
      createElement(tag) {
        assert.equal(tag, "section");
        return section;
      },
    },
  });
  try {
    const runner = createPhysicalHostRunner({
      host: { runtime },
      targetCatalog: createPhysicalHostTargetCatalog({ generation: 1, contributions: [contribution] }),
      presentationFor(scope) {
        assert.equal(scope, section);
        return {
          present(slot, view, handlers) { views.set(slot, { view, handlers }); },
          nextEvent() { return null; },
        };
      },
      ...callbacks,
    });
    assert.equal(runner, section);
  } finally {
    if (savedDocument) Object.defineProperty(globalThis, "document", savedDocument);
    else delete globalThis.document;
  }

  return {
    node(slot, key) {
      const node = views.get(slot).view.nodes.find((candidate) => candidate.key === key);
      assert.ok(node, `${slot} must present ${key}`);
      return node;
    },
    evidence() {
      return JSON.parse(views.get("physical-evidence").view.nodes
        .filter((node) => node.component === "code-block")
        .map((node) => node.value).join(""));
    },
    activate(name) {
      const { view, handlers } = views.get("physical-actions");
      const node = this.node("physical-actions", `physical-${name}`);
      assert.equal(node.state, "ready");
      assert.notEqual(node.action, null);
      assert.equal(view.actions[node.action].id, `physical.${name}`);
      handlers.onEvent({ action: view.actions[node.action].id, revision: view.revision });
    },
  };
}

test("physical target catalog admits exact evidence maxima and preserves finite refusals", () => {
  const admitted = targetContribution("boundary", {}, PHYSICAL_HOST_EVIDENCE_MAXIMA);
  const catalog = createPhysicalHostTargetCatalog({ generation: 2, contributions: [admitted] });
  const accepted = catalog.createAdapter({ targetId: "fixture/target-boundary" });
  assert.deepEqual(accepted.bounds, { maximumOperations: 5, ...PHYSICAL_HOST_EVIDENCE_MAXIMA });

  const cases = [
    [
      { terminal: "StaleCatalogGeneration", catalog_generation: 2 },
      () => createPhysicalHostTargetCatalog({ generation: 2, minimumGeneration: 2, contributions: [targetContribution()] }),
    ],
    [
      { terminal: "DuplicateIdentity", target_id: "fixture/target-one" },
      () => createPhysicalHostTargetCatalog({ generation: 2, contributions: [targetContribution(), targetContribution()] }),
    ],
    [
      { terminal: "CatalogBound", entries: 2, maximum_entries: 1 },
      () => createPhysicalHostTargetCatalog({
        generation: 2,
        bounds: { maximumEntries: 1 },
        contributions: [targetContribution("one"), targetContribution("two")],
      }),
    ],
    [
      { terminal: "IncompatibleContribution" },
      () => createPhysicalHostTargetCatalog({
        generation: 2,
        contributions: [targetContribution("operation", {}, {
          ...PHYSICAL_HOST_EVIDENCE_MAXIMA,
          maximumOperationEvidenceBytes: PHYSICAL_HOST_EVIDENCE_MAXIMA.maximumOperationEvidenceBytes + 1,
        })],
      }),
    ],
    [
      { terminal: "IncompatibleContribution" },
      () => createPhysicalHostTargetCatalog({
        generation: 2,
        contributions: [targetContribution("retained", {}, {
          ...PHYSICAL_HOST_EVIDENCE_MAXIMA,
          maximumRetainedEvidenceBytes: PHYSICAL_HOST_EVIDENCE_MAXIMA.maximumRetainedEvidenceBytes + 1,
        })],
      }),
    ],
    [
      { terminal: "IncompatibleAdapter", target_id: "fixture/target-one" },
      () => createPhysicalHostTargetCatalog({
        generation: 2,
        contributions: [targetContribution("one", {
          target: { id: "fixture/wrong", label: "Wrong", model_id: "wrong", profile_id: "wrong" },
          modes: [],
          bounds: {},
        })],
      }).createAdapter({ targetId: "fixture/target-one" }),
    ],
  ];
  for (const [evidence, create] of cases) {
    assert.throws(create, (error) => {
      assert.equal(error.name, "PhysicalHostTargetCatalogRefusal");
      for (const [key, value] of Object.entries(evidence)) {
        assert.equal(error.evidence[key], value);
      }
      return true;
    });
  }
});

test("physical workflow cancels one bounded operation without accepting late truth", async () => {
  let release;
  let signal;
  let cancelCount = 0;
  const fixture = runnerFixture(targetContribution("cancellable", {
    obtain(context) {
      signal = context.signal;
      return new Promise((resolve) => {
        release = () => resolve({
          resultKind: "artifact",
          evidence: { schema: "fixture/late-obtainment@1", disposition: "late-success" },
        });
      });
    },
    async cancel() { cancelCount += 1; },
  }));

  fixture.activate("cancel");
  const cancelled = fixture.evidence();
  assert.equal(cancelled.phase, "terminal");
  assert.equal(cancelled.cancellations, 1);
  assert.equal(cancelled.obtainment, null);
  assert.equal(cancelled.terminal.operation, "obtain");
  assert.equal(cancelled.terminal.terminal, "Cancelled");
  assert.equal(signal.aborted, true);
  release();
  // Drain the controlled promise completion once, without polling or retries.
  await nextTurn();
  assert.deepEqual(fixture.evidence(), cancelled);
  assert.equal(fixture.node("physical-progress", "physical-stage-obtain").value, "waiting");
  assert.equal(cancelCount, 1);
});

test("physical workflow refuses oversized operation evidence before accepting its result", async () => {
  const fixture = runnerFixture(targetContribution("evidence-bound", {
    async obtain() {
      return {
        resultKind: "artifact",
        evidence: { schema: "fixture/oversized@1", payload: "x".repeat(1024) },
      };
    },
  }, { maximumOperationEvidenceBytes: 512 }));
  await nextTurn();
  const evidence = fixture.evidence();
  assert.equal(evidence.terminal.terminal, "EvidenceBound");
  assert.equal(evidence.terminal.operation, "obtain");
  assert.equal(evidence.phase, "terminal");
  assert.equal(evidence.admitted_operations, 1);
  assert.equal(evidence.obtainment, null);
});

test("physical workflow delegates the exact observed join to its owning Body product", async () => {
  const join = Object.freeze({
    schema: "fixture/join@1",
    spore_id: "spore/delegated",
    image_id: "image/delegated",
    invitation_id: "invitation/delegated",
    body_id: "body/delegated",
    host_id: "host/delegated",
    boot_id: "boot/delegated",
    observed_at_millis: 1,
  });
  let calls = 0;
  let changed = 0;
  let candidate;
  const fixture = runnerFixture(targetContribution("delegated-admission", {
    async obtain() { return { resultKind: "artifact", evidence: { schema: "fixture/obtainment@1" } }; },
    async bind() { return { prepared: { spore_id: join.spore_id }, evidence: { schema: "fixture/binding@1" } }; },
    async realize() { return { terminal: "FixtureRealized", evidence: { schema: "fixture/realization@1" } }; },
    async observe() { return { join, evidence: { schema: "fixture/observation@1", ...join } }; },
  }, { maximumOperationEvidenceBytes: 2048, maximumRetainedEvidenceBytes: 8192 }), {
    async admitJoin(value) {
      calls += 1;
      candidate = value;
      return { schema: "fixture/admission@1", membership_revision: 7, offer_count: 2 };
    },
    onBodyChanged() { changed += 1; },
  });

  await nextTurn();
  for (const action of ["bind", "realize", "observe", "admit"]) {
    fixture.activate(action);
    await nextTurn();
  }
  assert.equal(calls, 1);
  assert.equal(changed, 1);
  assert.strictEqual(candidate, join);
  assert.equal(fixture.node("physical-progress", "physical-stage-admit").value, "revision 7");
  assert.match(fixture.node("physical-status", "physical-status").text, /2 current offers are ready/);
  assert.equal(fixture.evidence().admission.membership_revision, 7);
});
