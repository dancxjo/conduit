#!/usr/bin/env node

import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { basename, join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

export const OCI_LAYOUT_VERSION = "1.0.0";
export const OCI_INDEX = "application/vnd.oci.image.index.v1+json";
export const OCI_MANIFEST = "application/vnd.oci.image.manifest.v1+json";
export const CONDUIT_ARTIFACT = "application/vnd.conduit.release-artifact.v1";
export const CONDUIT_BUILD_MANIFEST = "application/vnd.conduit.build-manifest.v3+json";
export const CONDUIT_PROOF = "application/vnd.conduit.proof-receipt.v1+json";
export const INTOTO = "application/vnd.in-toto+json";
const EMPTY_CONFIG = "application/vnd.oci.empty.v1+json";
const MAX_METADATA_BYTES = 1024 * 1024;

export async function exportReleaseToOci(config, output) {
  validateConfig(config);
  const outputRoot = resolve(output);
  await mkdir(join(outputRoot, "blobs", "sha256"), { recursive: true });
  const artifact = await readFile(resolve(config.artifact.path));
  const buildManifest = await boundedFile(config.buildManifestPath, "build manifest");
  const proofReceipt = await boundedFile(config.proofReceiptPath, "proof receipt");
  const artifactLayer = await putBlob(outputRoot, artifact, config.artifact.mediaType);
  if (config.artifact.sha256 && artifactLayer.digest !== `sha256:${config.artifact.sha256}`) {
    throw new Error("artifact bytes disagree with the supplied ArtifactId digest");
  }
  const empty = await putJson(outputRoot, {}, EMPTY_CONFIG);
  const releaseManifest = await putJson(outputRoot, {
    schemaVersion: 2,
    mediaType: OCI_MANIFEST,
    artifactType: CONDUIT_ARTIFACT,
    config: empty,
    layers: [artifactLayer],
    annotations: {
      "org.opencontainers.image.title": basename(config.artifact.path),
      "org.opencontainers.image.revision": config.source.digest.sha1,
      "org.conduit.artifact-id": config.artifact.id,
      "org.conduit.build-id": config.build.id,
    },
  }, OCI_MANIFEST, CONDUIT_ARTIFACT);

  const provenance = statement(config, artifactLayer);
  const provenanceLayer = await putJson(outputRoot, provenance, INTOTO);
  const buildLayer = await putBlob(outputRoot, buildManifest, CONDUIT_BUILD_MANIFEST);
  const proofLayer = await putBlob(outputRoot, proofReceipt, CONDUIT_PROOF);
  const referrers = [];
  for (const [artifactType, layer] of [
    [INTOTO, provenanceLayer],
    [CONDUIT_BUILD_MANIFEST, buildLayer],
    [CONDUIT_PROOF, proofLayer],
  ]) {
    referrers.push(await putJson(outputRoot, {
      schemaVersion: 2,
      mediaType: OCI_MANIFEST,
      artifactType,
      config: empty,
      layers: [layer],
      subject: releaseManifest,
    }, OCI_MANIFEST, artifactType));
  }
  await writeFile(join(outputRoot, "oci-layout"), `${canonical({ imageLayoutVersion: OCI_LAYOUT_VERSION })}\n`, { flag: "wx" });
  await writeFile(join(outputRoot, "index.json"), `${canonical({
    schemaVersion: 2,
    mediaType: OCI_INDEX,
    manifests: [releaseManifest, ...referrers],
  })}\n`, { flag: "wx" });
  return { artifact: artifactLayer, manifest: releaseManifest, referrers };
}

export async function verifyOciRelease(output) {
  const root = resolve(output);
  const layout = JSON.parse(await readFile(join(root, "oci-layout"), "utf8"));
  if (layout.imageLayoutVersion !== OCI_LAYOUT_VERSION) throw new Error("unsupported OCI layout");
  const index = JSON.parse(await readFile(join(root, "index.json"), "utf8"));
  if (index.schemaVersion !== 2 || index.mediaType !== OCI_INDEX || index.manifests?.length !== 4) {
    throw new Error("OCI index does not contain one artifact and three bounded referrers");
  }
  const manifests = [];
  for (const descriptor of index.manifests) manifests.push(await verifiedJson(root, descriptor));
  for (const manifest of manifests) {
    await verifiedBlob(root, manifest.config);
    if (!Array.isArray(manifest.layers) || manifest.layers.length !== 1) throw new Error("OCI artifact manifest must have one bounded layer");
    await verifiedBlob(root, manifest.layers[0]);
  }
  const [release, ...referrers] = manifests;
  if (release.artifactType !== CONDUIT_ARTIFACT || release.subject) throw new Error("invalid release manifest");
  const releaseDescriptor = index.manifests[0];
  for (const referrer of referrers) {
    if (referrer.subject?.digest !== releaseDescriptor.digest
        || referrer.subject?.size !== releaseDescriptor.size
        || referrer.subject?.mediaType !== releaseDescriptor.mediaType) {
      throw new Error("referrer does not bind the exact release manifest");
    }
  }
  const types = new Set(referrers.map((item) => item.artifactType));
  for (const type of [INTOTO, CONDUIT_BUILD_MANIFEST, CONDUIT_PROOF]) {
    if (!types.has(type)) throw new Error(`missing ${type} referrer`);
  }
  const artifact = await verifiedBlob(root, release.layers[0]);
  const provenanceManifest = referrers.find((item) => item.artifactType === INTOTO);
  const provenance = JSON.parse(await verifiedBlob(root, provenanceManifest.layers[0]));
  if (provenance._type !== "https://in-toto.io/Statement/v1"
      || provenance.predicateType !== "https://slsa.dev/provenance/v1"
      || provenance.subject?.[0]?.digest?.sha256 !== digest(artifact)) {
    throw new Error("SLSA statement does not bind the exact artifact bytes");
  }
  return { artifactDigest: `sha256:${digest(artifact)}`, manifestDigest: releaseDescriptor.digest };
}

export async function prepareConduitosExport(releasePath, configPath, proofPath, sourceSha, invocationId) {
  if (!/^[0-9a-f]{40}$/.test(sourceSha)) throw new Error("source commit is invalid");
  const release = JSON.parse(await boundedFile(releasePath, "ConduitOS release manifest"));
  if (release?.schema !== "conduit.conduitos/creche-release@1"
      || release.target_id !== "conduitos/x86_64/pc"
      || release.artifact_role !== "product-host"
      || !/^image:sha256:[0-9a-f]{64}$/.test(release.image_id ?? "")
      || !/^sha256:[0-9a-f]{64}$/.test(release.artifact?.sha256 ?? "")
      || release.image_id !== `image:${release.artifact.sha256}`
      || release.boot_claimed !== false || release.physical_proof_claimed !== false) {
    throw new Error("ConduitOS release manifest is not the exact bounded x86_64 product receipt");
  }
  const releaseRoot = resolve(releasePath, "..");
  const artifactPath = join(releaseRoot, release.artifact.path);
  const proof = {
    schema: "conduit.proof/release-artifact-digest@1",
    proof_class: "source-capability",
    disposition: "release-bytes-digest-verified",
    artifact_id: release.image_id,
    artifact_sha256: release.artifact.sha256,
    build_id: release.build_id,
    source_commit: sourceSha,
    does_not_prove: ["installation", "boot", "physical-execution", "membership", "authority"],
  };
  await writeFile(resolve(proofPath), `${canonical(proof)}\n`, { flag: "wx" });
  const config = {
    artifact: { path: artifactPath, id: release.image_id, sha256: release.artifact.sha256.slice(7), mediaType: "application/vnd.conduit.conduitos.iso" },
    build: {
      id: release.build_id,
      type: "https://github.com/dancxjo/conduit/build-types/conduitos@v1",
      builderId: release.builder_adapter,
      invocationId,
    },
    source: { uri: "git+https://github.com/dancxjo/conduit", digest: { sha1: sourceSha } },
    buildManifestPath: resolve(releasePath), proofReceiptPath: resolve(proofPath),
  };
  await writeFile(resolve(configPath), `${canonical(config)}\n`, { flag: "wx" });
  return config;
}

function statement(config, artifact) {
  const metadata = { invocationId: config.build.invocationId };
  if (config.build.startedOn) metadata.startedOn = config.build.startedOn;
  if (config.build.finishedOn) metadata.finishedOn = config.build.finishedOn;
  return {
    _type: "https://in-toto.io/Statement/v1",
    subject: [{ name: config.artifact.id, digest: { sha256: artifact.digest.slice(7) } }],
    predicateType: "https://slsa.dev/provenance/v1",
    predicate: {
      buildDefinition: {
        buildType: config.build.type,
        externalParameters: { artifactId: config.artifact.id, buildId: config.build.id },
        internalParameters: {},
        resolvedDependencies: [{ uri: config.source.uri, digest: { sha1: config.source.digest.sha1 } }],
      },
      runDetails: { builder: { id: config.build.builderId }, metadata },
    },
  };
}

function validateConfig(config) {
  for (const [label, value] of [
    ["artifact path", config?.artifact?.path], ["ArtifactId", config?.artifact?.id],
    ["artifact media type", config?.artifact?.mediaType], ["BuildId", config?.build?.id],
    ["build type", config?.build?.type], ["builder id", config?.build?.builderId],
    ["invocation id", config?.build?.invocationId], ["source URI", config?.source?.uri],
    ["build manifest", config?.buildManifestPath], ["proof receipt", config?.proofReceiptPath],
  ]) if (typeof value !== "string" || value.length === 0 || value.length > 2048) throw new Error(`${label} is invalid`);
  if (!/^[0-9a-f]{40}$/.test(config.source?.digest?.sha1 ?? "")) throw new Error("source digest is not an exact SHA-1 commit");
  if (config.artifact.sha256 && !/^[0-9a-f]{64}$/.test(config.artifact.sha256)) throw new Error("artifact digest is invalid");
}

async function boundedFile(path, label) {
  const bytes = await readFile(resolve(path));
  if (bytes.length === 0 || bytes.length > MAX_METADATA_BYTES) throw new Error(`${label} exceeds its finite bound`);
  JSON.parse(bytes);
  return bytes;
}

async function putJson(root, value, mediaType, artifactType) {
  return putBlob(root, Buffer.from(`${canonical(value)}\n`), mediaType, artifactType);
}

async function putBlob(root, bytes, mediaType, artifactType) {
  const hex = digest(bytes);
  const path = join(root, "blobs", "sha256", hex);
  try { await writeFile(path, bytes, { flag: "wx" }); } catch (error) {
    if (error.code !== "EEXIST" || digest(await readFile(path)) !== hex) throw error;
  }
  return { mediaType, digest: `sha256:${hex}`, size: bytes.length, ...(artifactType ? { artifactType } : {}) };
}

async function verifiedJson(root, descriptor) {
  return JSON.parse(await verifiedBlob(root, descriptor));
}

async function verifiedBlob(root, descriptor) {
  if (!/^sha256:[0-9a-f]{64}$/.test(descriptor?.digest ?? "") || !Number.isSafeInteger(descriptor?.size)) throw new Error("invalid OCI descriptor");
  const bytes = await readFile(join(root, "blobs", "sha256", descriptor.digest.slice(7)));
  if (bytes.length !== descriptor.size || `sha256:${digest(bytes)}` !== descriptor.digest) throw new Error("OCI blob digest mismatch");
  return bytes;
}

function digest(bytes) { return createHash("sha256").update(bytes).digest("hex"); }
function canonical(value) {
  if (Array.isArray(value)) return `[${value.map(canonical).join(",")}]`;
  if (value && typeof value === "object") return `{${Object.keys(value).sort().map((key) => `${JSON.stringify(key)}:${canonical(value[key])}`).join(",")}}`;
  return JSON.stringify(value);
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  const [command, configPath, output, sourceSha, invocationId] = process.argv.slice(2);
  if (command === "export" && configPath && output) {
    await exportReleaseToOci(JSON.parse(await readFile(configPath, "utf8")), output);
  } else if (command === "verify" && configPath && !output) {
    await verifyOciRelease(configPath);
  } else if (command === "prepare-conduitos" && configPath && output && sourceSha && invocationId) {
    await prepareConduitosExport(configPath, `${output}.config.json`, `${output}.proof.json`, sourceSha, invocationId);
  } else throw new Error("usage: oci-release-export.mjs export CONFIG OUTPUT | verify OCI_LAYOUT");
}
