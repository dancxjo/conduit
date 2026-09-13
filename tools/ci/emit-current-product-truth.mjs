#!/usr/bin/env node

import { writeFile } from "node:fs/promises";
import { buildProductTruth } from "./product-truth.mjs";

const [
  outputPath,
  repository,
  developmentCommit,
  acceptedSourceCommit,
  acceptedMainCommit,
  pullRequestText,
  pagesUrl,
  publicationRunUrl,
  productProofRunUrl,
] = process.argv.slice(2);

if (!productProofRunUrl) {
  throw new Error("usage: emit-current-product-truth.mjs OUTPUT REPOSITORY DEV_SHA SOURCE_SHA MAIN_SHA PR PAGES_URL PUBLICATION_RUN PRODUCT_PROOF_RUN");
}

const pullRequest = Number(pullRequestText);
const truth = buildProductTruth({
  repository,
  development: {
    commit: developmentCommit,
    integration_run_url: `https://github.com/${repository}/commit/${developmentCommit}/checks`,
  },
  accepted_release: {
    source_commit: acceptedSourceCommit,
    main_commit: acceptedMainCommit,
    pull_request: pullRequest,
  },
  publication: {
    release_main_commit: acceptedMainCommit,
    pages_url: pagesUrl,
    deployment_url: publicationRunUrl,
  },
  evidence: [
    {
      surface: "accepted release",
      proof_class: "released-product",
      receipt_url: `https://github.com/${repository}/pull/${pullRequest}`,
      commit: acceptedMainCommit,
    },
    {
      surface: "browser products",
      proof_class: "live-browser",
      receipt_url: productProofRunUrl,
      commit: acceptedSourceCommit,
    },
    {
      surface: "ConduitOS visual journey",
      proof_class: "freestanding-emulator",
      receipt_url: productProofRunUrl,
      commit: acceptedSourceCommit,
    },
  ],
});

await writeFile(outputPath, `${JSON.stringify(truth, null, 2)}\n`, { flag: "wx" });
