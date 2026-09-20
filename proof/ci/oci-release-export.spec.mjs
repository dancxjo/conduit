import assert from "node:assert/strict";
import { mkdtemp, readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { createHash } from "node:crypto";

import { CONDUIT_PROOF, exportReleaseToOci, prepareConduitosExport, verifyOciRelease } from "../../tools/ci/oci-release-export.mjs";

const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");

async function fixture() {
  const root = await mkdtemp(join(tmpdir(), "conduit-oci-"));
  const artifactPath = join(root, "conduitos.iso");
  const artifact = Buffer.from("exact ConduitOS release bytes\n");
  await writeFile(artifactPath, artifact);
  const buildManifestPath = join(root, "build-manifest.json");
  await writeFile(buildManifestPath, JSON.stringify({ schema: "conduit.host/target-build-manifest@3", build_id: "build:exact" }));
  const proofReceiptPath = join(root, "proof.json");
  await writeFile(proofReceiptPath, JSON.stringify({ schema: "conduit.proof/conduitos@1", proof_class: "freestanding-emulator" }));
  return { root, artifact, config: {
    artifact: { path: artifactPath, id: `image:sha256:${sha256(artifact)}`, sha256: sha256(artifact), mediaType: "application/vnd.conduit.conduitos.iso" },
    build: { id: "build:exact", type: "https://github.com/dancxjo/conduit/build-types/conduitos@v1", builderId: "https://github.com/dancxjo/conduit/actions/workflows/tour-products.yml", invocationId: "run/42" },
    source: { uri: "git+https://github.com/dancxjo/conduit", digest: { sha1: "a".repeat(40) } },
    buildManifestPath, proofReceiptPath,
  } };
}

test("one exact release exports with separately typed subject referrers", async () => {
  const { root, artifact, config } = await fixture();
  const output = join(root, "layout");
  const result = await exportReleaseToOci(config, output);
  assert.equal(result.artifact.digest, `sha256:${sha256(artifact)}`);
  assert.equal(result.referrers.length, 3);
  assert(result.referrers.some((item) => item.artifactType === CONDUIT_PROOF));
  assert.deepEqual(await verifyOciRelease(output), { artifactDigest: result.artifact.digest, manifestDigest: result.manifest.digest });
});

test("the layout is deterministic and a mutable index annotation cannot redefine identity", async () => {
  const { root, config } = await fixture();
  const first = join(root, "first");
  const second = join(root, "second");
  const a = await exportReleaseToOci(config, first);
  const b = await exportReleaseToOci(config, second);
  assert.deepEqual(a, b);
  const indexPath = join(second, "index.json");
  const index = JSON.parse(await readFile(indexPath));
  index.manifests[0].annotations = { "org.opencontainers.image.ref.name": "mutable-latest" };
  await writeFile(indexPath, JSON.stringify(index));
  const verified = await verifyOciRelease(second);
  assert.equal(verified.artifactDigest, a.artifact.digest);
});

test("fresh verification rejects changed artifact and referrer bytes", async () => {
  const { root, config } = await fixture();
  const output = join(root, "layout");
  const exported = await exportReleaseToOci(config, output);
  const artifactBlob = join(output, "blobs", "sha256", exported.artifact.digest.slice(7));
  await writeFile(artifactBlob, "changed");
  await assert.rejects(verifyOciRelease(output), /digest mismatch/);

  const clean = join(root, "clean-layout");
  await exportReleaseToOci(config, clean);
  const index = JSON.parse(await readFile(join(clean, "index.json")));
  const proofDescriptor = index.manifests.find((item) => item.artifactType === CONDUIT_PROOF);
  const proofManifest = JSON.parse(await readFile(join(clean, "blobs", "sha256", proofDescriptor.digest.slice(7))));
  await writeFile(join(clean, "blobs", "sha256", proofManifest.layers[0].digest.slice(7)), "changed proof");
  await assert.rejects(verifyOciRelease(clean), /digest mismatch/);
});

test("export refuses a claimed digest that disagrees with ArtifactId bytes", async () => {
  const { root, config } = await fixture();
  config.artifact.sha256 = "f".repeat(64);
  await assert.rejects(exportReleaseToOci(config, join(root, "layout")), /ArtifactId digest/);
});

test("the ConduitOS adapter preserves BuildId and ArtifactId without claiming boot", async () => {
  const root = await mkdtemp(join(tmpdir(), "conduit-oci-release-"));
  const artifact = Buffer.from("iso");
  await writeFile(join(root, "conduitos-x86_64-pc.iso"), artifact);
  const releasePath = join(root, "conduitos-x86_64-pc-release.json");
  const buildManifestPath = join(root, "build-manifest.json");
  await writeFile(releasePath, JSON.stringify({
    schema: "conduit.conduitos/creche-release@1", target_id: "conduitos/x86_64/pc",
    artifact_role: "product-host", image_id: `image:sha256:${sha256(artifact)}`,
    build_id: "build:exact", builder_adapter: "conduit-host-conduitos/build-x86_64@1",
    artifact: { path: "conduitos-x86_64-pc.iso", sha256: `sha256:${sha256(artifact)}` },
    boot_claimed: false, physical_proof_claimed: false,
  }));
  await writeFile(buildManifestPath, JSON.stringify({
    schema: "conduit.host/target-build-manifest@3", target: "conduitos/x86_64/pc",
    artifact_role: "product-host", build_id: "build:exact",
    image_id: `image:sha256:${sha256(artifact)}`,
  }));
  const configPath = join(root, "export.config.json");
  const proofPath = join(root, "export.proof.json");
  const config = await prepareConduitosExport(releasePath, buildManifestPath, configPath, proofPath, "a".repeat(40), "run/42");
  assert.equal(config.artifact.id, `image:sha256:${sha256(artifact)}`);
  assert.equal(config.build.id, "build:exact");
  const proof = JSON.parse(await readFile(proofPath));
  assert.equal(proof.proof_class, "source-capability");
  assert.deepEqual(proof.does_not_prove, ["installation", "boot", "physical-execution", "membership", "authority"]);
});
