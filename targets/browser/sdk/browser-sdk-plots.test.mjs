import test from "node:test";
import assert from "node:assert/strict";
import { BrowserPlot, birthBrowserBody, recoverBrowserBody, reviewBrowserPlots } from "./browser-sdk-plots.mjs";
import { InvalidLifecycleError } from "./browser-sdk.mjs";

const source = 'clock { tick: presentation/tick }.';
const plot = {
  name: "clock",
  source,
  source_document_id: "sha256:source",
  checked_plot_id: "sha256:checked",
  required_kinds: ["presentation/tick"],
};
const bodySnapshot = (revision) => ({ schema: "conduit.workspace/body@1", evidence: {
  schema: "conduit.body/biography-evidence@2", body_id: "body/1", body: { workload_revision: revision, workset: { plots: [{ source_document_id: "sha256:source", checked_plot_id: "sha256:checked" }] } },
  membership: { revision: 0 }, records: [], wakes: [],
}, current_host_offers: [] });

test("Plot check projects Rust source and checked identities without parsing in JavaScript", async () => {
  const bridge = { crecheReviewedInventory(input) {
    assert.equal(input, source);
    return { status: 0, outputJson: { schema: "conduit.creche/reviewed-plot-inventory@2", source_document_id: "sha256:source", plots: [plot] } };
  } };
  const checked = await new BrowserPlot(source, bridge).check();
  assert.equal(checked.schema, "conduit.browser/checked-source@1");
  assert.equal(checked.sourceDocumentId, "sha256:source");
  assert.equal(checked.plots[0].checkedPlotId, "sha256:checked");
  assert.deepEqual(checked.requirements.kinds, ["presentation/tick"]);
  assert.equal(checked.plots[0].documentSource, source);
});

test("invalid source returns canonical diagnostics and parser spans without prose scraping", async () => {
  const diagnostic = { code: "CND-SYN-001", message: "unclosed Plot", span: { start: 0, end: 5, line: 1, column: 1, endLine: 1, endColumn: 6 } };
  const bridge = { crecheReviewedInventory: () => ({
    status: -459,
    outputJson: { schema: "conduit.creche/plot-check-refusal@1", code: "PlotSyntaxInvalid", message: "source is invalid", diagnostics: [diagnostic] },
  }) };
  const checked = await new BrowserPlot("clock {", bridge).check();
  assert.equal(checked.ok, false);
  assert.equal(checked.refusal.code, "PlotSyntaxInvalid");
  assert.deepEqual(checked.diagnostics[0], diagnostic);
});

test("Plot Patchbay exposes immutable Rust-checked Front constraints without creating a Body", () => {
  const calls = [];
  const plot = new BrowserPlot(source, { projectPatchbay(input, sequence) {
    calls.push({ input, sequence });
    return { status: 0, outputJson: {
      schema: "conduit.patchbay/checked-plot-projection@1",
      source_document_id: "sha256:source",
      checked_plot_id: "sha256:checked",
      visible_expanded_plot_id: "sha256:expanded",
      realization_expanded_plot_id: "sha256:expanded",
      source_proposal_id: "proposal/1",
      sequence: Number(sequence), plot_name: "clock", realization: "direct",
      front_inputs: [{ port_id: "code", info_kind: "value/text", temporal: "value", value_contract: {
        value_kind: "value/text", maximum_bytes: 8,
        constraints: [{ CanonicalMembership: { members: [[65, 66, 49, 50]] } }],
      } }],
      front_outputs: [], gears: [], cords: [], realization_gears: [], realization_cords: [],
      realization_backs: [], diagnostics: [],
    } };
  } });

  const first = plot.patchbay();
  const second = plot.patchbay();
  assert.deepEqual(calls, [{ input: source, sequence: 1n }, { input: source, sequence: 2n }]);
  assert.equal(first.front_inputs[0].value_contract.maximum_bytes, 8);
  assert.equal(second.sequence, 2);
  assert.equal(Object.isFrozen(first.front_inputs[0].value_contract.constraints[0]), true);
});

test("Host workload review returns bounded realization requirements without creating a Plan or acquiring resources", async () => {
  const checked = await new BrowserPlot(source, { crecheReviewedInventory: () => ({ status: 0, outputJson: { source_document_id: "sha256:source", plots: [plot] } }) }).check();
  const bridge = { crecheReviewInitialWorkload(request) {
    assert.equal(request.host, "host/1");
    assert.equal(request.boot, "boot/1");
    assert.deepEqual(request.initialPlots, [{ name: "clock", source_document_id: "sha256:source", checked_plot_id: "sha256:checked" }]);
    return { status: 0, outputJson: {
      schema: "conduit.creche/plot-workload-review@1",
      review: { schema: "conduit.creche/initial-workload-review@1", proposed_hosts: [], reviewed_realization_count: 1, body_plan_created: false, play_created: false, authority_acquired: false, resources_acquired: false },
      requirements: { kinds: ["presentation/tick"], resources: [{ host_id: "host/1", resource_class_id: "resource/dom@1", units: 1 }], capabilities: [{ host_id: "host/1", capability_id: "browser/dom@1", active_instances: 1 }] },
    } };
  } };
  const review = await reviewBrowserPlots({ bridge, host: "host/1", boot: "boot/1", plots: checked.plots });
  assert.deepEqual(review.requirements.kinds, ["presentation/tick"]);
  assert.deepEqual(review.requirements.resources[0], { hostId: "host/1", resourceClassId: "resource/dom@1", units: 1 });
  assert.equal(review.bodyPlanCreated, false);
  assert.equal(review.resourcesAcquired, false);
});

test("workset changes send canonical checked identity and exact observed workload revision", async () => {
  const requests = [];
  const snapshot = bodySnapshot(7);
  const bridge = {
    crecheAdmitSourceInteraction: () => ({ status: 0, outputJson: {} }),
    crecheBirth: () => ({ status: 0, outputJson: { body_id: "body/1" } }),
    crecheAttachHere: () => ({ status: 0, outputJson: { body_id: "body/1" } }),
    workspaceRequest(request) {
      requests.push(request);
      return { status: 0, outputJson: request.action === "Arrive" ? snapshot : snapshot };
    },
    crecheReviewedInventory() { throw new Error("checked values must be reused"); },
  };
  const checked = Object.freeze({ schema: "conduit.browser/checked-plot@1", name: "clock", source, documentSource: source, sourceDocumentId: "sha256:source", checkedPlotId: "sha256:checked" });
  const body = await birthBrowserBody({ bridge, host: "host/1", boot: "boot/1", membership: { advertisement: () => ({}) }, name: "Clock", plots: [checked], sequence: () => 1 });
  await body.install(checked);
  const change = requests.find((request) => request.action === "ChangeWorkset");
  assert.equal(change.expected_revision, 7);
  assert.equal(change.plot.source_document_id, "sha256:source");
  assert.equal(change.plot.checked_plot_id, "sha256:checked");
  assert.equal(change.source, source);
});

test("Body Patchbay uses the Rust projection and binds it to exact Body and Boot truth", async () => {
  const snapshot = bodySnapshot(0);
  let projectedSource;
  let projectedSequence;
  const bridge = {
    crecheAdmitSourceInteraction: () => ({ status: 0, outputJson: {} }),
    crecheBirth: () => ({ status: 0, outputJson: { body_id: "body/1" } }),
    crecheAttachHere: () => ({ status: 0, outputJson: { body_id: "body/1" } }),
    workspaceRequest: () => ({ status: 0, outputJson: snapshot }),
    projectPatchbay(input, sequence) {
      projectedSource = input;
      projectedSequence = sequence;
      return { status: 0, outputJson: {
        schema: "conduit.patchbay/checked-plot-projection@1",
        sequence: Number(sequence),
        source_document_id: "sha256:source",
        checked_plot_id: "sha256:checked",
        visible_expanded_plot_id: "sha256:expanded",
        realization_expanded_plot_id: "sha256:expanded",
        source_proposal_id: "proposal/1",
        plot_name: "clock",
        realization: "direct",
        front_inputs: [{ port_id: "count", info_kind: "value/count", temporal: "value", value_contract: {
          value_kind: "value/count", maximum_bytes: 8, constraints: [{ UnsignedRange: {
            minimum: 1, maximum: 4, minimum_endpoint: "Exclusive", maximum_endpoint: "Inclusive",
          } }],
        } }],
        front_outputs: [],
        gears: [{ gear_id: "clock/tick", kind_id: "presentation/tick", inputs: [], outputs: [] }],
        cords: [], realization_gears: [], realization_cords: [], realization_backs: [], diagnostics: [],
      } };
    },
  };
  const checked = { schema: "conduit.browser/checked-plot@1", name: "clock", source, documentSource: source, sourceDocumentId: "sha256:source", checkedPlotId: "sha256:checked" };
  const body = await birthBrowserBody({ bridge, host: "host/1", boot: "boot/1", membership: { advertisement: () => ({}) }, name: "Clock", plots: [checked], sequence: () => 1 });

  const patchbay = await body.patchbay();
  assert.equal(projectedSource, source);
  assert.equal(projectedSequence, 1n);
  assert.equal(patchbay.bodyId, "body/1");
  assert.equal(patchbay.hostId, "host/1");
  assert.equal(patchbay.bootId, "boot/1");
  assert.equal(patchbay.topology.checked_plot_id, "sha256:checked");
  assert.equal(Object.isFrozen(patchbay.topology.gears), true);
  assert.equal(patchbay.topology.front_inputs[0].value_contract.constraints[0].UnsignedRange.minimum, 1);
  assert.equal(Object.isFrozen(patchbay.topology.front_inputs), true);
  assert.equal(Object.isFrozen(patchbay.topology.front_inputs[0].value_contract.constraints), true);
});

test("workset refusal retains the exact expected revision", async () => {
  const bridge = { workspaceRequest(request) {
    return request.action === "ChangeWorkset"
      ? { status: -2, outputJson: { code: "StaleWorkload", message: "revision changed" } }
      : { status: 0, outputJson: bodySnapshot(4) };
  }, crecheAdmitSourceInteraction: () => ({ status: 0, outputJson: {} }), crecheBirth: () => ({ status: 0, outputJson: { body_id: "body/1" } }), crecheAttachHere: () => ({ status: 0, outputJson: { body_id: "body/1" } }) };
  const checked = { schema: "conduit.browser/checked-plot@1", name: "clock", source, documentSource: source, sourceDocumentId: "sha256:source", checkedPlotId: "sha256:checked" };
  const body = await birthBrowserBody({ bridge, host: "host/1", boot: "boot/1", membership: { advertisement: () => ({}) }, name: "Clock", plots: [checked], sequence: () => 1 });
  await assert.rejects(body.install(checked), (error) => error.code === "StaleWorkload"
    && error instanceof InvalidLifecycleError
    && error.identities.bodyId === "body/1"
    && error.identities.hostId === "host/1"
    && error.identities.bootId === "boot/1"
    && error.identities.expectedWorkloadRevision === "4");
});

test("durable birth retains runtime-owned Body truth with its freshly checked source", async () => {
  const writes = [];
  const snapshot = bodySnapshot(0);
  const bridge = {
    crecheAdmitSourceInteraction: () => ({ status: 0, outputJson: {} }),
    crecheBirth: () => ({ status: 0, outputJson: { body_id: "body/1" } }),
    crecheAttachHere: () => ({ status: 0, outputJson: { body_id: "body/1" } }),
    workspaceRequest(request) {
      if (request.action === "Durable") return { status: 0, outputJson: { ...snapshot, admission: { body_id: "body/1" } } };
      return { status: 0, outputJson: snapshot };
    },
  };
  const storage = { readJson: async () => null, writeJson: async (key, value) => writes.push({ key, value }) };
  const checked = { schema: "conduit.browser/checked-plot@1", name: "clock", source, documentSource: source, sourceDocumentId: "sha256:source", checkedPlotId: "sha256:checked" };
  const body = await birthBrowserBody({ bridge, host: "host/1", boot: "boot/1", membership: { advertisement: () => ({}) }, storage, name: "Clock", plots: [checked], sequence: () => 1 });
  assert.equal(body.id, "body/1");
  assert.equal(writes.length, 1);
  assert.equal(writes[0].key, "body-continuity");
  assert.equal(writes[0].value.schema, "conduit.browser/body-continuity@1");
  assert.equal(writes[0].value.source, source);
  assert.equal(writes[0].value.durable.evidence.body_id, "body/1");
});

test("recover rechecks retained Plots and restores the same Body under the fresh Boot without a Play", async () => {
  const requests = [];
  const snapshot = bodySnapshot(3);
  const durable = { ...snapshot, admission: { body_id: "body/1" } };
  const bridge = {
    crecheReviewedInventory(input) {
      assert.equal(input, source);
      return { status: 0, outputJson: { source_document_id: "sha256:source", plots: [plot] } };
    },
    workspaceRequest(request) {
      requests.push(request);
      if (request.action === "Restore") return { status: 0, outputJson: snapshot };
      if (request.action === "Current") return { status: 0, outputJson: snapshot };
      if (request.action === "Durable") return { status: 0, outputJson: durable };
      throw new Error(`unexpected ${request.action}`);
    },
  };
  const writes = [];
  const storage = {
    readJson: async () => ({ schema: "conduit.browser/body-continuity@1", source, durable }),
    writeJson: async (key, value) => writes.push({ key, value }),
  };
  const body = await recoverBrowserBody({
    bridge, host: "host/1", boot: "boot/fresh", membership: { advertisement: () => ({ host_id: "host/1", boot_id: "boot/fresh" }) }, storage,
  });
  assert.equal(body.id, "body/1");
  assert.equal(body.receipt.schema, "conduit.browser/body-recovery@1");
  assert.equal(body.receipt.boot_id, "boot/fresh");
  await body.current();
  assert.equal(requests[0].action, "Restore");
  assert.equal(requests[0].boot_id, "boot/fresh");
  assert.equal(requests.some(({ action }) => action === "Arrive"), false);
  assert.equal(writes[0].value.durable.evidence.body_id, "body/1");
});

test("birth refuses to overwrite a retained Body that requires recovery", async () => {
  const storage = { readJson: async () => ({ schema: "conduit.browser/body-continuity@1" }) };
  await assert.rejects(birthBrowserBody({ bridge: {}, host: "host/1", boot: "boot/2", membership: { advertisement: () => ({}) }, storage, name: "Replacement", plots: [{}], sequence: () => 1 }),
    (error) => error.code === "RetainedBodyRequiresRecovery" && error.identities.hostId === "host/1");
});

test("recover refuses retained Plot identities that no longer match canonical checking", async () => {
  const snapshot = bodySnapshot(3);
  snapshot.evidence.body.workset.plots[0].checked_plot_id = "sha256:stale";
  const bridge = { crecheReviewedInventory: () => ({ status: 0, outputJson: { source_document_id: "sha256:source", plots: [plot] } }) };
  const storage = { readJson: async () => ({ schema: "conduit.browser/body-continuity@1", source, durable: { ...snapshot, admission: { body_id: "body/1" } } }) };
  await assert.rejects(recoverBrowserBody({ bridge, host: "host/1", boot: "boot/fresh", membership: { advertisement: () => ({}) }, storage }),
    (error) => error.code === "DurablePlotIdentityMismatch" && error.identities.bootId === "boot/fresh");
});

test("closing a Body settles admitted selection before fencing stale handles", async () => {
  const requests = [];
  const snapshot = bodySnapshot(0);
  const bridge = {
    crecheAdmitSourceInteraction: () => ({ status: 0, outputJson: {} }),
    crecheBirth: () => ({ status: 0, outputJson: { body_id: "body/1" } }),
    crecheAttachHere: () => ({ status: 0, outputJson: { body_id: "body/1" } }),
    workspaceRequest(request) { requests.push(request); return { status: 0, outputJson: snapshot }; },
  };
  const checked = Object.freeze({ schema: "conduit.browser/checked-plot@1", name: "clock", source,
    documentSource: source, sourceDocumentId: "sha256:source", checkedPlotId: "sha256:checked" });
  const body = await birthBrowserBody({ bridge, host: "host/1", boot: "boot/1",
    membership: { advertisement: () => ({}) }, name: "Clock", plots: [checked], sequence: () => 1 });
  const selection = body.select(checked);
  const close = body.close();
  assert.equal(body.close(), close);
  await selection;
  await close;
  assert.deepEqual(requests.find(request => request.action === "SelectPlot").plot,
    { source_document_id: "sha256:source", checked_plot_id: "sha256:checked" });
  const count = requests.length;
  for (const operation of [() => body.current(), () => body.wake(), () => body.select(checked), () => body.install(checked)]) {
    await assert.rejects(operation, error => error.code === "BodyClosed");
  }
  assert.equal(requests.length, count, "closed handles cannot mutate or read the runtime");
});

test("bundled edits retain reviewed presentation profiles and Patchbay projects exact foreground source", async () => {
  const original = { slug: "clock", entry: "clock", title: "Clock", source, presentation_profile: 1 };
  const bundle = JSON.stringify({ schema: "conduit.creche/reviewed-plot-bundle@2", plots: [original] });
  const added = { ...plot, name: "second", source: "second {}", source_document_id: "source/second", checked_plot_id: "checked/second" };
  let retained = bundle;
  let projected;
  let mismatched = false;
  const snapshot = bodySnapshot(0);
  snapshot.foreground = { source_document_id: "sha256:source", checked_plot_id: "sha256:checked" };
  snapshot.evidence.body.workset.plots.push({ source_document_id: added.source_document_id, checked_plot_id: added.checked_plot_id });
  const bridge = {
    crecheAdmitSourceInteraction: () => ({ status: 0, outputJson: {} }),
    crecheBirth: () => ({ status: 0, outputJson: { body_id: "body/1" } }),
    crecheAttachHere: () => ({ status: 0, outputJson: { body_id: "body/1" } }),
    crecheReviewedInventory: () => ({ status: 0, outputJson: { plots: [plot, added] } }),
    workspaceRequest(request) {
      if (request.action === "ChangeWorkset") retained = request.source;
      return { status: 0, outputJson: snapshot };
    },
    projectPatchbay(input) {
      projected = input;
      return { status: 0, outputJson: {
        schema: "conduit.patchbay/checked-plot-projection@1",
        source_document_id: mismatched ? added.source_document_id : plot.source_document_id,
        checked_plot_id: mismatched ? added.checked_plot_id : plot.checked_plot_id,
        front_inputs: [], front_outputs: [], gears: [], cords: [], realization_gears: [],
        realization_cords: [], realization_backs: [], diagnostics: [],
      } };
    },
  };
  const checked = (entry, documentSource) => ({ schema: "conduit.browser/checked-plot@1", name: entry.name, source: entry.source, documentSource,
    sourceDocumentId: entry.source_document_id, checkedPlotId: entry.checked_plot_id });
  const body = await birthBrowserBody({ bridge, host: "host/1", boot: "boot/1", membership: { advertisement: () => ({}) },
    name: "Bundle", plots: [checked(plot, bundle)], sequence: () => 1 });
  const incoming = { slug: "second", entry: "second", source: added.source, presentation_profile: 3 };
  const incomingBundle = JSON.stringify({ schema: "conduit.creche/reviewed-plot-bundle@2", plots: [incoming] });
  await body.install(checked(added, incomingBundle));
  assert.deepEqual(JSON.parse(retained).plots, [original, incoming]);
  await body.patchbay();
  assert.equal(projected, source);
  mismatched = true;
  await assert.rejects(body.patchbay(), error => error.code === "PatchbayPlotIdentityMismatch");
  snapshot.foreground = null;
  await assert.rejects(body.patchbay(), error => error.code === "PatchbayPlotIdentityMismatch");
});
