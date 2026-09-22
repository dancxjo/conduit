import assert from "node:assert/strict";
import test from "node:test";

import { ontologyCasingViolations } from "../../tools/ci/ontology-common-nouns.mjs";

test("maintained prose treats ontology terms as common nouns", () => {
  assert.deepEqual(ontologyCasingViolations(), []);
});
