import assert from "node:assert/strict";
import test from "node:test";
import { authoringBasis, configurationText, configurationValue, requireAuthoringSuccess, snapshotOrder } from "../../targets/browser/patchbay-workbench/assets/authoring.js";

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

test("physical configuration submits authored source to Rust admission", () => {
  assert.deepEqual(configurationValue({ Bool: false }, "true"), { Bool: true });
  assert.throws(() => configurationValue({ Bool: false }, "1"));
  const quantity = { Quantity: { canonical_value: [], source: "1Hz" } };
  assert.equal(configurationText(quantity), "1Hz");
  assert.deepEqual(configurationValue(quantity, "1Qm"), { QuantitySource: "1Qm" });
  assert.deepEqual(configurationValue(quantity, "9223372036854775807Hz"), { QuantitySource: "9223372036854775807Hz" });
  assert.deepEqual(configurationValue({ Unit: { source: "Hz" } }, "kHz"), { UnitSource: "kHz" });
  assert.deepEqual(configurationValue({ TemperatureDifference: { source: "9°F" } }, "5K"), { TemperatureDifferenceSource: "5K" });
});


test("refused, stale, failed and missing acknowledgements cannot report a successful edit", () => {
  for (const disposition of ["Refused(StaleFace)", "Refused(IncompatiblePorts)", "Failed", "Cancelled", undefined]) {
    assert.throws(() => requireAuthoringSuccess({ interaction: { last_disposition: disposition } }));
  }
  const acknowledged = { interaction: { last_disposition: "Succeeded" } };
  assert.equal(requireAuthoringSuccess(acknowledged), acknowledged);
});


test("late snapshot responses cannot roll back a Face or its current interaction", () => {
  const current = { revision: 7, interaction: { revision: 4 } };
  assert.equal(snapshotOrder({ revision: 6, interaction: { revision: 999 } }, current), -1);
  assert.equal(snapshotOrder({ revision: 7, interaction: { revision: 3 } }, current), -1);
  assert.equal(snapshotOrder(structuredClone(current), current), 0);
  assert.equal(snapshotOrder({ revision: 7, interaction: { revision: 5 } }, current), 1);
  assert.equal(snapshotOrder({ revision: 8, interaction: { revision: 0 } }, current), 1);
  assert.equal(snapshotOrder(current, null), 1);
});
