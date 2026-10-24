import assert from "node:assert/strict";
import test from "node:test";
import { ConfigurationValueError, authoringBasis, configurationHelp, configurationText, configurationValue } from "../../targets/browser/patchbay-workbench/assets/authoring.js";

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


test("field requirements retain exact bounds and distinguish bytes, choices and units", () => {
  assert.equal(configurationHelp({ rule: { I64Range: { minimum: "-9223372036854775808", maximum: "9223372036854775807" } }, value: { I64: 0 } }),
    "Whole number from -9223372036854775808 to 9223372036854775807.");
  assert.equal(configurationHelp({ rule: { U64Range: { minimum: 0, maximum: "18446744073709551615" } }, value: { U64: 0 } }),
    "Whole number from 0 to 18446744073709551615.");
  assert.equal(configurationHelp({ rule: { DurationMillis: { minimum: 1, maximum: 200 } }, value: { U64: 1 } }),
    "Whole milliseconds from 1 to 200.");
  assert.match(configurationHelp({ rule: { TextBytes: { maximum: 128 } }, value: { Text: "" } }), /128 UTF-8 bytes/);
  assert.match(configurationHelp({ rule: { DataLabel: { maximum: 64 } }, value: { Text: "" } }), /Required data name.*64 UTF-8 bytes/);
  assert.equal(configurationHelp({ rule: { TextOneOf: { values: ["a", "b"] } }, value: { Text: "a" } }), "Choose one of the listed values.");
  assert.equal(configurationHelp({ rule: "Any", value: { Bool: false } }), "Choose true or false.");
  assert.equal(configurationHelp({ rule: "Quantity", value: {} }), "Enter a quantity with its unit.");
  assert.match(configurationHelp({ rule: { QuantityRange: {} }, value: {} }), /Exact bounds are in the rule details/);
});


test("wire overflow refuses exactly at the input boundary", () => {
  for (const raw of ["9223372036854775808", "-9223372036854775809"]) {
    assert.throws(() => configurationValue({ I64: 0 }, raw), ConfigurationValueError);
  }
  for (const raw of ["-1", "18446744073709551616"]) {
    assert.throws(() => configurationValue({ U64: 0 }, raw), ConfigurationValueError);
  }
});
