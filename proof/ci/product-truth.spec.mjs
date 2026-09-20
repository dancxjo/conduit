import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdir, mkdtemp, readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";

import {
  buildProductTruth,
  MAX_EVIDENCE_RECEIPTS,
  validateProductTruth,
} from "../../tools/ci/product-truth.mjs";

const sha = (digit) => digit.repeat(40);
function input() {
  return {
    repository: "dancxjo/conduit",
    development: { commit: sha("d"), integration_run_url: "https://example/dev-run" },
    accepted_release: { source_commit: sha("a"), main_commit: sha("b"), pull_request: 42 },
    publication: {
      release_main_commit: sha("c"),
      pages_url: "https://example/current/",
      deployment_url: "https://example/deployment",
    },
    evidence: [
      {
        surface: "conduitos/x86_64",
        proof_class: "freestanding-emulator",
        receipt_url: "https://example/emulator-receipt",
        commit: sha("a"),
      },
      {
        surface: "tour/browser",
        proof_class: "live-browser",
        receipt_url: "https://example/browser-receipt",
        commit: sha("a"),
      },
    ],
  };
}

test("exact identities derive both lag claims without promoting proof classes", () => {
  const truth = buildProductTruth(input());
  assert.deepEqual(truth.lag, {
    accepted_release_behind_development: true,
    publication_behind_accepted_release: true,
  });
  assert.deepEqual(truth.evidence.map((item) => item.proof_class), [
    "freestanding-emulator",
    "live-browser",
  ]);
  assert.equal(validateProductTruth(truth), true);
});

test("malformed identities proof classes and lag claims refuse", () => {
  const malformed = input();
  malformed.development.commit = "dev";
  assert.throws(() => buildProductTruth(malformed), /exact commit/);
  const promoted = input();
  promoted.evidence[0].proof_class = "proof-ish";
  assert.throws(() => buildProductTruth(promoted), /proof class/);
  const falseLag = structuredClone(buildProductTruth(input()));
  falseLag.lag.accepted_release_behind_development = false;
  assert.throws(() => validateProductTruth(falseLag), /lag claims/);
});

test("CLI writes one bounded machine-readable artifact without overwriting", async () => {
  const directory = await mkdtemp(join(tmpdir(), "conduit-product-truth-"));
  const inputPath = join(directory, "input.json");
  const outputPath = join(directory, "product-truth.json");
  await writeFile(inputPath, JSON.stringify(input()));
  const first = spawnSync(process.execPath, ["tools/ci/product-truth.mjs", inputPath, outputPath], {
    cwd: process.cwd(), encoding: "utf8",
  });
  assert.equal(first.status, 0, first.stderr);
  assert.equal(validateProductTruth(JSON.parse(await readFile(outputPath, "utf8"))), true);
  const second = spawnSync(process.execPath, ["tools/ci/product-truth.mjs", inputPath, outputPath], {
    cwd: process.cwd(), encoding: "utf8",
  });
  assert.notEqual(second.status, 0);
});

test("the artifact refuses unbounded or ambiguous evidence", () => {
  const tooMany = input();
  tooMany.evidence = Array.from({ length: MAX_EVIDENCE_RECEIPTS + 1 }, (_, index) => ({
    surface: `surface-${index}`,
    proof_class: "source-capability",
    receipt_url: `https://example/receipt-${index}`,
    commit: sha("a"),
  }));
  assert.throws(() => buildProductTruth(tooMany), /evidence bound/);

  const duplicate = input();
  duplicate.evidence[1].surface = duplicate.evidence[0].surface;
  assert.throws(() => buildProductTruth(duplicate), /duplicate evidence surface/);

  const oversized = input();
  oversized.publication.pages_url = "x".repeat(2_049);
  assert.throws(() => buildProductTruth(oversized), /exceeds its bound/);
});

test("the release emitter records exact current identities and proof receipts", async () => {
  const directory = await mkdtemp(join(tmpdir(), "conduit-current-truth-"));
  const outputPath = join(directory, "product-truth.json");
  const supplyChain = join(directory, "supply-chain");
  await mkdir(join(supplyChain, "blobs", "sha256"), { recursive: true });
  const release = Buffer.from(JSON.stringify({ annotations: {
    "org.conduit.artifact-id": `image:sha256:${"e".repeat(64)}`,
    "org.conduit.build-id": "build:exact",
  } }));
  const releaseDigest = createHash("sha256").update(release).digest("hex");
  await writeFile(join(supplyChain, "blobs", "sha256", releaseDigest), release);
  await writeFile(join(supplyChain, "index.json"), JSON.stringify({ manifests: [{ digest: `sha256:${releaseDigest}` }] }));
  const result = spawnSync(process.execPath, [
    "tools/ci/emit-current-product-truth.mjs",
    outputPath,
    "dancxjo/conduit",
    sha("d"),
    sha("a"),
    sha("b"),
    "42",
    "https://dancxjo.github.io/conduit/",
    "https://example/publication-run",
    "https://example/product-proof-run",
    supplyChain,
  ], { cwd: process.cwd(), encoding: "utf8" });
  assert.equal(result.status, 0, result.stderr);
  const truth = JSON.parse(await readFile(outputPath, "utf8"));
  assert.equal(validateProductTruth(truth), true);
  assert.equal(truth.lag.accepted_release_behind_development, true);
  assert.deepEqual(truth.evidence.map(({ proof_class }) => proof_class), [
    "released-product", "live-browser", "freestanding-emulator",
  ]);
  assert.equal(truth.supply_chain[0].artifact_id, `image:sha256:${"e".repeat(64)}`);
  assert.equal(truth.supply_chain[0].build_id, "build:exact");
  assert.match(truth.supply_chain[0].index_digest, /^sha256:[0-9a-f]{64}$/);
});
