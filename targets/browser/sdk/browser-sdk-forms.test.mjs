import test from "node:test";
import assert from "node:assert/strict";
import { BrowserForm, birthBrowserBody, reviewBrowserForms } from "./browser-sdk-forms.mjs";
import { InvalidLifecycleError } from "./browser-sdk.mjs";

const source = 'clock { tick: presentation/tick }.';
const form = {
  name: "clock",
  source,
  source_document_id: "sha256:source",
  checked_form_id: "sha256:checked",
  required_kinds: ["presentation/tick"],
};
const bodySnapshot = (revision) => ({ schema: "conduit.workspace/body@1", evidence: {
  schema: "conduit.body/biography-evidence@2", body_id: "body/1", body: { workload_revision: revision },
  membership: { revision: 0 }, records: [], wakes: [],
}, current_host_offers: [] });

test("Form check projects Rust source and checked identities without parsing in JavaScript", async () => {
  const bridge = { crecheReviewedInventory(input) {
    assert.equal(input, source);
    return { status: 0, outputJson: { schema: "conduit.creche/reviewed-form-inventory@1", source_document_id: "sha256:source", forms: [form] } };
  } };
  const checked = await new BrowserForm(source, bridge).check();
  assert.equal(checked.schema, "conduit.browser/checked-source@1");
  assert.equal(checked.sourceDocumentId, "sha256:source");
  assert.equal(checked.forms[0].checkedFormId, "sha256:checked");
  assert.deepEqual(checked.requirements.kinds, ["presentation/tick"]);
  assert.equal(checked.forms[0].documentSource, source);
});

test("invalid source returns canonical diagnostics and parser spans without prose scraping", async () => {
  const diagnostic = { code: "CND-SYN-001", message: "unclosed Form", span: { start: 0, end: 5, line: 1, column: 1, endLine: 1, endColumn: 6 } };
  const bridge = { crecheReviewedInventory: () => ({
    status: -459,
    outputJson: { schema: "conduit.creche/form-check-refusal@1", code: "FormSyntaxInvalid", message: "source is invalid", diagnostics: [diagnostic] },
  }) };
  const checked = await new BrowserForm("clock {", bridge).check();
  assert.equal(checked.ok, false);
  assert.equal(checked.refusal.code, "FormSyntaxInvalid");
  assert.deepEqual(checked.diagnostics[0], diagnostic);
});

test("Host workload review returns bounded realization requirements without creating a Plan or acquiring resources", async () => {
  const checked = await new BrowserForm(source, { crecheReviewedInventory: () => ({ status: 0, outputJson: { source_document_id: "sha256:source", forms: [form] } }) }).check();
  const bridge = { crecheReviewInitialWorkload(request) {
    assert.equal(request.host, "host/1");
    assert.equal(request.boot, "boot/1");
    assert.deepEqual(request.initialForms, [{ name: "clock", source_document_id: "sha256:source", checked_form_id: "sha256:checked" }]);
    return { status: 0, outputJson: {
      schema: "conduit.creche/form-workload-review@1",
      review: { schema: "conduit.creche/initial-workload-review@1", proposed_hosts: [], reviewed_realization_count: 1, body_plan_created: false, play_created: false, authority_acquired: false, resources_acquired: false },
      requirements: { kinds: ["presentation/tick"], resources: [{ host_id: "host/1", resource_class_id: "resource/dom@1", units: 1 }], capabilities: [{ host_id: "host/1", capability_id: "browser/dom@1", active_instances: 1 }] },
    } };
  } };
  const review = await reviewBrowserForms({ bridge, host: "host/1", boot: "boot/1", forms: checked.forms });
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
  const checked = Object.freeze({ schema: "conduit.browser/checked-form@1", name: "clock", source, documentSource: source, sourceDocumentId: "sha256:source", checkedFormId: "sha256:checked" });
  const body = await birthBrowserBody({ bridge, host: "host/1", boot: "boot/1", membership: { advertisement: () => ({}) }, name: "Clock", forms: [checked], sequence: () => 1 });
  await body.install(checked);
  const change = requests.find((request) => request.action === "ChangeWorkset");
  assert.equal(change.expected_revision, 7);
  assert.equal(change.form.source_document_id, "sha256:source");
  assert.equal(change.form.checked_form_id, "sha256:checked");
  assert.equal(change.source, source);
});

test("workset refusal retains the exact expected revision", async () => {
  const bridge = { workspaceRequest(request) {
    return request.action === "ChangeWorkset"
      ? { status: -2, outputJson: { code: "StaleWorkload", message: "revision changed" } }
      : { status: 0, outputJson: bodySnapshot(4) };
  }, crecheAdmitSourceInteraction: () => ({ status: 0, outputJson: {} }), crecheBirth: () => ({ status: 0, outputJson: { body_id: "body/1" } }), crecheAttachHere: () => ({ status: 0, outputJson: { body_id: "body/1" } }) };
  const checked = { schema: "conduit.browser/checked-form@1", name: "clock", source, documentSource: source, sourceDocumentId: "sha256:source", checkedFormId: "sha256:checked" };
  const body = await birthBrowserBody({ bridge, host: "host/1", boot: "boot/1", membership: { advertisement: () => ({}) }, name: "Clock", forms: [checked], sequence: () => 1 });
  await assert.rejects(body.install(checked), (error) => error.code === "StaleWorkload"
    && error instanceof InvalidLifecycleError
    && error.identities.bodyId === "body/1"
    && error.identities.hostId === "host/1"
    && error.identities.bootId === "boot/1"
    && error.identities.expectedWorkloadRevision === "4");
});
