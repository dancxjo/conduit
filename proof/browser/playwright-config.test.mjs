import assert from "node:assert/strict";
import test from "node:test";

import { browserProofPort, browserProofShard } from "./playwright.config.mjs";

test("browser proof ports remain bounded decimal loopback ports", () => {
  assert.equal(browserProofPort(), "4173");
  assert.equal(browserProofPort("4174"), "4174");
  for (const invalid of ["", "80", "1023", "65536", "4173/path", "localhost:4173", " 4173"]) {
    assert.throws(() => browserProofPort(invalid), /CONDUIT_BROWSER_HOST_PORT/);
  }
});

test("browser proof shards select one bounded result child", () => {
  assert.equal(browserProofShard(), "default");
  assert.equal(browserProofShard("creche-machines_2"), "creche-machines_2");
  for (const invalid of ["", "../escape", "tour/output", "Tour", "x".repeat(49)]) {
    assert.throws(() => browserProofShard(invalid), /CONDUIT_BROWSER_PROOF_SHARD/);
  }
});
