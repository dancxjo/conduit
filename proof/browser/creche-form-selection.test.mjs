import assert from "node:assert/strict";
import test from "node:test";

import {
  encodedFormSelection,
  openFormSelection,
  persistedFormSelection,
  searchForms,
  setFormSelected,
  toggleForm,
} from "../../products/creche/browser/creche-form-selection.mjs";
import { exportBodyEvidence } from "../../products/creche/browser/creche-graduation.mjs";
import { initialFormSelectionNotice, selectedCanonicalSource } from "../../products/creche/browser/creche-lifecycle.mjs";

const inventory = Object.freeze({
  schema: "conduit.creche/reviewed-form-inventory@1",
  source_document_id: "source/current",
  maximum_selection: 3,
  forms: [
    { name: "clock", title: "Clock", source: "form clock {}\n", source_document_id: "source/clock", checked_form_id: "checked/clock", required_kinds: ["time/every"] },
    { name: "desk_telegraph", title: "Desk Telegraph", source: "form desk_telegraph {}\n", source_document_id: "source/telegraph", checked_form_id: "checked/telegraph", required_kinds: ["presentation/text", "text/literal"] },
    { name: "memory_lantern", title: "Memory Lantern", source: "form memory_lantern {}\n", source_document_id: "source/lantern", checked_form_id: "checked/lantern", required_kinds: ["presentation/text"] },
  ],
});

test("native checkbox values set selection idempotently", () => {
  const once = setFormSelected(inventory, [], "clock", true);
  assert.strictEqual(setFormSelected(inventory, once, "clock", true), once);
  assert.deepEqual(setFormSelected(inventory, once, "clock", false), []);
  assert.strictEqual(setFormSelected(inventory, [], "clock", false).length, 0);
  assert.throws(() => setFormSelected(inventory, [], "clock", "true"), /boolean/);
});

test("browse, search, add, remove, and exact encoding share one bounded inventory", () => {
  assert.deepEqual(searchForms(inventory, "text").map(({ name }) => name), ["desk_telegraph", "memory_lantern"]);
  let selected = toggleForm(inventory, [], "clock");
  selected = toggleForm(inventory, selected, "desk_telegraph");
  assert.deepEqual(JSON.parse(encodedFormSelection(selected)), [
    { name: "clock", source_document_id: "source/clock", checked_form_id: "checked/clock" },
    { name: "desk_telegraph", source_document_id: "source/telegraph", checked_form_id: "checked/telegraph" },
  ]);
  selected = toggleForm(inventory, selected, "clock");
  assert.deepEqual(selected.map(({ name }) => name), ["desk_telegraph"]);
  assert.equal(
    selectedCanonicalSource([inventory.forms[0], inventory.forms[1]]),
    "form clock {}\n\nform desk_telegraph {}",
  );
  assert.equal(selectedCanonicalSource([]), "");
});

test("restoration and Gallery handoff revalidate exact identities without privilege", () => {
  const retained = persistedFormSelection(inventory, [inventory.forms[0]]);
  retained.forms.push({ name: "memory_lantern", source_document_id: "source/old", checked_form_id: "checked/old" });
  const opened = openFormSelection(inventory, retained, inventory.forms[1]);
  assert.deepEqual(opened.selected.map(({ name }) => name), ["clock", "desk_telegraph"]);
  assert.equal(opened.refusals[0].disposition, "stale-form-identity");
  assert.equal(opened.refusals[0].origin, "restored");
  assert.strictEqual(opened.acceptedHandoff, inventory.forms[1]);
  assert.match(initialFormSelectionNotice(opened), /Desk Telegraph was revalidated and preselected from Gallery/);
  assert.match(initialFormSelectionNotice(opened), /no Body has been born/);

  const staleHandoff = openFormSelection(inventory, null, {
    name: "desk_telegraph",
    source_document_id: "source/stale",
    checked_form_id: "checked/telegraph",
  });
  assert.equal(staleHandoff.acceptedHandoff, null);
  assert.equal(staleHandoff.refusals[0].origin, "gallery-handoff");
  assert.equal(
    initialFormSelectionNotice(staleHandoff),
    "The Gallery Form handoff was stale or substituted and was not selected.",
  );
});

test("invalid, duplicate, and over-capacity state is explicit", () => {
  const duplicateInventory = { ...inventory, forms: [...inventory.forms, inventory.forms[0]], maximum_selection: 4 };
  assert.throws(() => openFormSelection(duplicateInventory), /duplicate/);
  assert.throws(() => openFormSelection(inventory, { schema: "wrong", forms: [] }), /malformed/);
  assert.throws(() => toggleForm({ ...inventory, maximum_selection: 0, forms: [] }, [], "clock"), /absent/);
  assert.throws(() => searchForms(inventory, "x".repeat(129)), /bound/);
});

test("graduation exports the exact bounded Body biography without mutation", async () => {
  const bodyId = "a".repeat(64);
  const biography = { schema: "conduit.body/biography-evidence@2", body_id: bodyId, records: [{ sequence: 1, sign_id: "sign/born", kind: { Born: {} } }] };
  const priorDocument = globalThis.document;
  const priorCreate = URL.createObjectURL;
  const priorRevoke = URL.revokeObjectURL;
  let exportedBlob;
  let download;
  let clicked = false;
  URL.createObjectURL = (blob) => { exportedBlob = blob; return "blob:body-evidence"; };
  URL.revokeObjectURL = (url) => assert.equal(url, "blob:body-evidence");
  globalThis.document = { createElement(tag) { assert.equal(tag, "a"); return { click() { clicked = true; }, set download(value) { download = value; }, set href(value) { assert.equal(value, "blob:body-evidence"); } }; } };
  try {
    exportBodyEvidence(biography);
    assert.equal(clicked, true);
    assert.equal(download, `conduit-body-${bodyId}.json`);
    assert.deepEqual(JSON.parse(await exportedBlob.text()), biography);
    assert.throws(() => exportBodyEvidence({ body_id: bodyId }), /unavailable/);
    assert.throws(() => exportBodyEvidence({ body_id: bodyId, records: [], padding: "x".repeat(65_536) }), /export bound/);
  } finally {
    globalThis.document = priorDocument;
    URL.createObjectURL = priorCreate;
    URL.revokeObjectURL = priorRevoke;
  }
});
