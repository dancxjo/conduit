import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import test from "node:test";
import {
  classifyHttpFailure,
  downloadExactArtifact,
} from "../../tools/ci/download-exact-artifact.mjs";

function response(status, value, headers = {}) {
  return {
    status,
    ok: status >= 200 && status < 300,
    headers: { get: (name) => headers[name.toLowerCase()] ?? null },
    text: async () => (typeof value === "string" ? value : JSON.stringify(value)),
    json: async () => value,
    arrayBuffer: async () => Buffer.from(value),
  };
}

function exactOptions(fetchImpl, bytes = Buffer.from("exact archive")) {
  return {
    repository: "dancxjo/conduit",
    runId: "42",
    name: "exact-artifact",
    expectedDigest: createHash("sha256").update(bytes).digest("hex"),
    token: "test-token",
    fetchImpl,
    sleep: async () => {},
  };
}

test("classified transient service failure retries then verifies exact bytes", async () => {
  const bytes = Buffer.from("exact archive");
  const replies = [
    response(403, "Error from intermediary with HTTP status code 403"),
    response(200, { artifacts: [{ id: 7, name: "exact-artifact", expired: false }] }),
    response(503, "unavailable"),
    response(200, { artifacts: [{ id: 7, name: "exact-artifact", expired: false }] }),
    response(200, bytes),
  ];
  const result = await downloadExactArtifact(exactOptions(async () => replies.shift(), bytes));
  assert.equal(result.attempts, 3);
  assert.deepEqual(result.retries, ["list-intermediary-403", "download-http-503"]);
  assert.deepEqual(result.bytes, bytes);
});

test("permanent authorization failure does not retry", async () => {
  let requests = 0;
  await assert.rejects(
    downloadExactArtifact(
      exactOptions(async () => {
        requests += 1;
        return response(403, { message: "Resource not accessible by integration" });
      }),
    ),
    /artifact list failed permanently: list-http-403/,
  );
  assert.equal(requests, 1);
});

test("missing exact producer is permanent", async () => {
  let requests = 0;
  await assert.rejects(
    downloadExactArtifact(
      exactOptions(async () => {
        requests += 1;
        return response(200, { artifacts: [] });
      }),
    ),
    /expected one live exact artifact/,
  );
  assert.equal(requests, 1);
});

test("digest mismatch does not retry or extract", async () => {
  let requests = 0;
  const replies = [
    response(200, { artifacts: [{ id: 7, name: "exact-artifact", expired: false }] }),
    response(200, Buffer.from("wrong archive")),
  ];
  await assert.rejects(
    downloadExactArtifact(
      exactOptions(async () => {
        requests += 1;
        return replies.shift();
      }),
    ),
    /artifact digest mismatch/,
  );
  assert.equal(requests, 2);
});

test("only classified intermediary 403 is transient", () => {
  assert.equal(
    classifyHttpFailure("download", response(403, "intermediary"), "intermediary").transient,
    true,
  );
  assert.equal(
    classifyHttpFailure("list", response(403, "intermediary"), "intermediary").transient,
    true,
  );
  assert.equal(
    classifyHttpFailure("download", response(403, "forbidden"), "forbidden").transient,
    false,
  );
});
