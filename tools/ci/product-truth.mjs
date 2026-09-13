#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";
import { pathToFileURL } from "node:url";

export const PRODUCT_TRUTH_SCHEMA = "conduit.product-truth/v1";
export const MAX_EVIDENCE_RECEIPTS = 32;
const MAX_TEXT_LENGTH = 2_048;
export const PROOF_CLASSES = Object.freeze(new Set([
  "source-capability",
  "deterministic-simulation",
  "hosted-execution",
  "live-browser",
  "freestanding-emulator",
  "firmware-execution",
  "live-transport",
  "physical-hil",
  "human-enactment",
  "released-product",
]));

export function buildProductTruth(input) {
  validateInput(input);
  const truth = {
    schema: PRODUCT_TRUTH_SCHEMA,
    repository: input.repository,
    development: { ...input.development },
    accepted_release: { ...input.accepted_release },
    publication: { ...input.publication },
    evidence: input.evidence.map((item) => ({ ...item })),
    lag: {
      accepted_release_behind_development:
        input.accepted_release.source_commit !== input.development.commit,
      publication_behind_accepted_release:
        input.publication.release_main_commit !== input.accepted_release.main_commit,
    },
  };
  return deepFreeze(truth);
}

export function validateProductTruth(truth) {
  if (truth?.schema !== PRODUCT_TRUTH_SCHEMA) throw new Error("unknown product-truth schema");
  validateInput(truth);
  const expected = buildProductTruth(truth);
  if (truth.lag?.accepted_release_behind_development
      !== expected.lag.accepted_release_behind_development
      || truth.lag?.publication_behind_accepted_release
      !== expected.lag.publication_behind_accepted_release) {
    throw new Error("product-truth lag claims disagree with exact identities");
  }
  return true;
}

function validateInput(input) {
  requiredText(input?.repository, "repository");
  sha(input?.development?.commit, "development commit");
  requiredText(input?.development?.integration_run_url, "development integration run");
  sha(input?.accepted_release?.source_commit, "accepted release source");
  sha(input?.accepted_release?.main_commit, "accepted release main commit");
  positiveInteger(input?.accepted_release?.pull_request, "accepted release pull request");
  sha(input?.publication?.release_main_commit, "publication release commit");
  requiredText(input?.publication?.pages_url, "publication URL");
  requiredText(input?.publication?.deployment_url, "publication deployment");
  if (!Array.isArray(input?.evidence) || input.evidence.length === 0) {
    throw new Error("product truth requires explicit evidence");
  }
  if (input.evidence.length > MAX_EVIDENCE_RECEIPTS) {
    throw new Error("product truth exceeds the evidence bound");
  }
  const surfaces = new Set();
  for (const item of input.evidence) {
    requiredText(item?.surface, "evidence surface");
    if (surfaces.has(item.surface)) throw new Error("duplicate evidence surface");
    surfaces.add(item.surface);
    if (!PROOF_CLASSES.has(item?.proof_class)) throw new Error("unknown evidence proof class");
    requiredText(item?.receipt_url, "evidence receipt");
    sha(item?.commit, "evidence commit");
  }
}

function requiredText(value, label) {
  if (typeof value !== "string" || value.trim() === "") throw new Error(`${label} is empty`);
  if (value.length > MAX_TEXT_LENGTH) throw new Error(`${label} exceeds its bound`);
}

function sha(value, label) {
  if (typeof value !== "string" || !/^[0-9a-f]{40}$/.test(value)) {
    throw new Error(`${label} is not an exact commit`);
  }
}

function positiveInteger(value, label) {
  if (!Number.isSafeInteger(value) || value <= 0) throw new Error(`${label} is invalid`);
}

function deepFreeze(value) {
  Object.freeze(value);
  for (const child of Object.values(value)) {
    if (child && typeof child === "object" && !Object.isFrozen(child)) deepFreeze(child);
  }
  return value;
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  const [, , inputPath, outputPath] = process.argv;
  if (!inputPath || !outputPath) throw new Error("usage: product-truth.mjs INPUT OUTPUT");
  const input = JSON.parse(await readFile(inputPath, "utf8"));
  const truth = buildProductTruth(input);
  await writeFile(outputPath, `${JSON.stringify(truth, null, 2)}\n`, { flag: "wx" });
}
