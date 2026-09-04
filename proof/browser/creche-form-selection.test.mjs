import assert from "node:assert/strict";
import test from "node:test";

import {
  encodedFormSelection,
  openFormSelection,
  persistedFormSelection,
  searchForms,
  setFormSelected,
  toggleForm,
} from "../../targets/browser/host/assets/creche-form-selection.mjs";

const inventory = Object.freeze({
  schema: "conduit.creche/reviewed-form-inventory@1",
  source_document_id: "source/current",
  maximum_selection: 3,
  forms: [
    { name: "clock", title: "Clock", source_document_id: "source/clock", checked_form_id: "checked/clock", required_kinds: ["time/every"] },
    { name: "desk_telegraph", title: "Desk Telegraph", source_document_id: "source/telegraph", checked_form_id: "checked/telegraph", required_kinds: ["presentation/text", "text/literal"] },
    { name: "memory_lantern", title: "Memory Lantern", source_document_id: "source/lantern", checked_form_id: "checked/lantern", required_kinds: ["presentation/text"] },
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
});

test("restoration and Gallery handoff revalidate exact identities without privilege", () => {
  const retained = persistedFormSelection(inventory, [inventory.forms[0]]);
  retained.forms.push({ name: "memory_lantern", source_document_id: "source/old", checked_form_id: "checked/old" });
  const opened = openFormSelection(inventory, retained, inventory.forms[1]);
  assert.deepEqual(opened.selected.map(({ name }) => name), ["clock", "desk_telegraph"]);
  assert.equal(opened.refusals[0].disposition, "stale-form-identity");
});

test("invalid, duplicate, and over-capacity state is explicit", () => {
  const duplicateInventory = { ...inventory, forms: [...inventory.forms, inventory.forms[0]], maximum_selection: 4 };
  assert.throws(() => openFormSelection(duplicateInventory), /duplicate/);
  assert.throws(() => openFormSelection(inventory, { schema: "wrong", forms: [] }), /malformed/);
  assert.throws(() => toggleForm({ ...inventory, maximum_selection: 0, forms: [] }, [], "clock"), /absent/);
  assert.throws(() => searchForms(inventory, "x".repeat(129)), /bound/);
});
