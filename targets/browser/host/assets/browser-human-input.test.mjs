import test from "node:test";
import assert from "node:assert/strict";
import { advanceInputSequence } from "./browser-human-input.mjs";

for (const [name, maximum] of [["pointer u32", 0xffff_ffff], ["button safe Number to u64", Number.MAX_SAFE_INTEGER]]) {
  test(`${name} emits its final wire value once and retires`, () => {
    assert.equal(advanceInputSequence(0, maximum), 1);
    assert.equal(advanceInputSequence(maximum - 1, maximum), maximum);
    assert.equal(advanceInputSequence(maximum, maximum), null);
    assert.equal(advanceInputSequence(null, maximum), undefined);
    assert.equal(advanceInputSequence(maximum + 1, maximum), undefined);
    assert.equal(advanceInputSequence(-1, maximum), undefined);
    assert.equal(advanceInputSequence(0.5, maximum), undefined);
  });
}
