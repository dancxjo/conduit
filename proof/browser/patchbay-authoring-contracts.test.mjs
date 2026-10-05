import assert from "node:assert/strict";
import test from "node:test";
import { authoringBasis, configurationText, configurationValue } from "../../targets/browser/patchbay-workbench/assets/authoring.js";

test("authoring queries correlate the complete source and checked basis", () => {
  const basis = { source_document_id: "source/a", source_revision: 1, checked_plot_id: "checked/a", expanded_plot_id: "expanded/a" };
  assert.equal(authoringBasis({ ...basis }), authoringBasis(basis));
  assert.equal(authoringBasis(null), null);
  for (const key of Object.keys(basis)) {
    assert.notEqual(authoringBasis({ ...basis, [key]: `${basis[key]}-changed` }), authoringBasis(basis));
  }
});

test("configuration controls preserve exact canonical integer values for checking", () => {
  assert.deepEqual(configurationValue({ I64: 0 }, "-9223372036854775808"), { I64: "-9223372036854775808" });
  assert.deepEqual(configurationValue({ U64: 0 }, "18446744073709551615"), { U64: "18446744073709551615" });
  assert.deepEqual(configurationValue({ I64: 0 }, "42"), { I64: 42 });
  assert.equal(configurationText({ U64: "18446744073709551615" }), "18446744073709551615");
  for (const value of ["1.5", "1e3", "NaN", ""]) {
    assert.throws(() => configurationValue({ I64: 0 }, value));
  }
});

test("configuration presentation neither coerces Boolean nor changes quantity units", () => {
  assert.deepEqual(configurationValue({ Bool: false }, "true"), { Bool: true });
  assert.throws(() => configurationValue({ Bool: false }, "1"));
  assert.deepEqual(configurationValue({ Quantity: { value: 1, unit: "Hertz" } }, "42"),
    { Quantity: { value: 42, unit: "Hertz" } });
  assert.throws(() => configurationValue({ Quantity: { value: 1, unit: "Hertz" } }, "9223372036854775807"));
});
