#!/usr/bin/env node

import { createHash } from "node:crypto";
import { readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
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
  supplyChainRoot,
] = process.argv.slice(2);

if (!supplyChainRoot) {
  throw new Error("usage: emit-current-product-truth.mjs OUTPUT REPOSITORY DEV_SHA SOURCE_SHA MAIN_SHA PR PAGES_URL PUBLICATION_RUN PRODUCT_PROOF_RUN SUPPLY_CHAIN_ROOT");
}

const indexBytes = await readFile(join(supplyChainRoot, "index.json"));
const index = JSON.parse(indexBytes);
const releaseDescriptor = index.manifests?.[0];
if (!/^sha256:[0-9a-f]{64}$/.test(releaseDescriptor?.digest ?? "")) throw new Error("OCI release manifest digest is absent");
const release = JSON.parse(await readFile(join(supplyChainRoot, "blobs", "sha256", releaseDescriptor.digest.slice(7))));
const artifactId = release.annotations?.["org.conduit.artifact-id"];
const buildId = release.annotations?.["org.conduit.build-id"];

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
  supply_chain: [{
    artifact_id: artifactId,
    build_id: buildId,
    representation: "oci-image-layout@1",
    manifest_digest: releaseDescriptor.digest,
    index_digest: `sha256:${createHash("sha256").update(indexBytes).digest("hex")}`,
    index_url: `${pagesUrl.replace(/\/?$/, "/")}supply-chain/conduitos-x86_64-pc/index.json`,
  }],
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
