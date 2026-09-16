import assert from "node:assert/strict";
import test from "node:test";
import { acquireHostRelease } from "../../products/creche/browser/creche-release-bundle.mjs";
import { openReleaseCatalog, createMemoryReleaseCache } from "../../products/creche/browser/creche-release-catalog.mjs";

const encoder = new TextEncoder();

test("a missing release manifest refuses before payload acquisition", async () => {
  const profile = {
    target_id: "std/x86_64/computer",
    package_id: "hosted-native@1",
    output: "native-bundle",
    builder_adapter: "conduit-host-hosted/build-native@1",
    deployment_adapter: "conduit-host-hosted/launch@1",
    manifest_path: "https://mirror.example/releases/hosted-linux-x86_64.json",
  };
  await assert.rejects(
    acquireHostRelease(profile, undefined, { fetcher: async () => new Response("missing", { status: 404 }) }),
    (error) => error.code === "ArtifactUnavailable"
      && error.evidence.operation === "obtain"
      && error.evidence.authority_requested === false
      && error.evidence.artifact_work_started === false
      && error.evidence.external_work_started === false,
  );
});

test("selected target alone is fetched, verified, and reused from immutable cache", async () => {
  const payload = encoder.encode("reviewed-host-bundle");
  const payloadDigest = await sha256(payload);
  const bundleDigest = await sha256(encoder.encode(`conduit.release/host-bundle-content@1\0host.bin\0${payloadDigest}\n`));
  const manifest = {
    schema: "conduit.release/host-bundle@1",
    target_id: "std/x86_64/computer",
    fabrication_package_id: "hosted-native@1",
    output: "native-bundle",
    builder_adapter: "conduit-host-hosted/build-native@1",
    deployment_adapter: "conduit-host-hosted/launch@1",
    source_identity: "git:reviewed",
    bundle_sha256: bundleDigest,
    files: [{ path: "host.bin", bytes: payload.byteLength, sha256: payloadDigest, media_type: "application/octet-stream" }],
  };
  const manifestBytes = encoder.encode(JSON.stringify(manifest));
  const manifestDigest = await sha256(manifestBytes);
  const catalog = {
    schema: "conduit.release/catalog@1",
    generation: 7,
    catalog_id: "",
    entries: [{
      target_id: manifest.target_id,
      package_id: manifest.fabrication_package_id,
      output: manifest.output,
      builder_adapter: manifest.builder_adapter,
      deployment_adapter: manifest.deployment_adapter,
      manifest: { path: "selected/manifest.json", bytes: manifestBytes.byteLength, sha256: manifestDigest },
    }, {
      target_id: "conduitos/x86_64/pc",
      package_id: "conduitos-image@1",
      output: "disk-image",
      builder_adapter: "unused/build@1",
      deployment_adapter: "unused/load@1",
      manifest: { path: "unselected/manifest.json", bytes: 10, sha256: `sha256:${"8".repeat(64)}` },
    }],
  };
  catalog.catalog_id = await catalogId(catalog);
  const catalogBytes = encoder.encode(JSON.stringify(catalog));
  const resources = new Map([
    ["https://mirror.example/releases/catalog.json", catalogBytes],
    ["https://mirror.example/releases/selected/manifest.json", manifestBytes],
    ["https://mirror.example/releases/selected/host.bin", payload],
  ]);
  const requests = [];
  const fetcher = async (input) => {
    const url = String(input);
    requests.push(url);
    const bytes = resources.get(url);
    return bytes
      ? new Response(bytes, { status: 200, headers: { "content-type": "application/octet-stream" } })
      : new Response("missing", { status: 404 });
  };
  const cache = createMemoryReleaseCache();
  const release = await openReleaseCatalog({
    source: "https://mirror.example/releases/catalog.json", fetcher, cache,
  });
  const profile = {
    target_id: manifest.target_id,
    release_catalog_key: manifest.target_id,
    package_id: manifest.fabrication_package_id,
    output: manifest.output,
    builder_adapter: manifest.builder_adapter,
    deployment_adapter: manifest.deployment_adapter,
  };
  const first = await release.acquire(profile);
  assert.deepEqual(first.payloads[0].bytes, payload);
  assert.equal(requests.some((url) => url.includes("unselected")), false);
  const fetchedAfterFirst = requests.length;
  await release.acquire(profile);
  assert.equal(requests.length, fetchedAfterFirst, "manifest and payload came from the verified immutable cache");
});

test("catalog cannot relabel one artifact as another target", async () => {
  const catalog = {
    schema: "conduit.release/catalog@1", generation: 2,
    catalog_id: "",
    entries: [{
      target_id: "target/a", package_id: "package/a", output: "image",
      builder_adapter: "build/a", deployment_adapter: "deploy/a",
      manifest: { path: "a.json", bytes: 2, sha256: `sha256:${"b".repeat(64)}` },
    }],
  };
  catalog.catalog_id = await catalogId(catalog);
  const fetcher = async () => new Response(JSON.stringify(catalog), { status: 200 });
  const release = await openReleaseCatalog({ source: "https://mirror.example/catalog.json", fetcher });
  await assert.rejects(
    release.acquire({ target_id: "target/a", release_catalog_key: "target/a", package_id: "package/b", output: "image", builder_adapter: "build/a", deployment_adapter: "deploy/a" }),
    (error) => error.code === "CatalogMismatch" && error.evidence.body_binding_started === false,
  );
});

test("target-owned image validators can resolve one exact catalog artifact", async () => {
  const image = encoder.encode("reviewed-conduitos-image");
  const imageDigest = await sha256(image);
  const manifest = { artifact: { path: "host.iso", bytes: image.byteLength, sha256: imageDigest } };
  const manifestBytes = encoder.encode(JSON.stringify(manifest));
  const descriptor = {
    target_id: "conduitos/x86_64/pc",
    package_id: "conduitos-image@1",
    output: "disk-image",
    builder_adapter: "conduit-host-conduitos/build-x86_64@1",
    deployment_adapter: "conduit-host-conduitos/boot-x86_64@1",
  };
  const catalog = {
    schema: "conduit.release/catalog@1",
    generation: 9,
    catalog_id: "",
    entries: [{
      ...descriptor,
      manifest: {
        path: "conduitos/manifest.json",
        bytes: manifestBytes.byteLength,
        sha256: await sha256(manifestBytes),
      },
    }],
  };
  catalog.catalog_id = await catalogId(catalog);
  const resources = new Map([
    ["https://mirror.example/catalog.json", encoder.encode(JSON.stringify(catalog))],
    ["https://mirror.example/conduitos/manifest.json", manifestBytes],
    ["https://mirror.example/conduitos/host.iso", image],
  ]);
  const requests = [];
  const fetcher = async (input) => {
    const url = String(input); requests.push(url);
    const bytes = resources.get(url);
    return bytes ? new Response(bytes, { status: 200 }) : new Response("missing", { status: 404 });
  };
  const release = await openReleaseCatalog({
    source: "https://mirror.example/catalog.json",
    fetcher,
    cache: createMemoryReleaseCache(),
  });
  const resolved = await release.resolve(descriptor);
  assert.deepEqual(JSON.parse(new TextDecoder().decode(resolved.manifest)), manifest);
  assert.deepEqual(await resolved.acquire(manifest.artifact, 80 * 1024 * 1024), image);
  assert.deepEqual(requests, [
    "https://mirror.example/catalog.json",
    "https://mirror.example/conduitos/manifest.json",
    "https://mirror.example/conduitos/host.iso",
  ]);
});

async function sha256(bytes) {
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", bytes));
  return `sha256:${Array.from(digest, (byte) => byte.toString(16).padStart(2, "0")).join("")}`;
}

async function catalogId(catalog) {
  return sha256(encoder.encode(JSON.stringify({
    schema: catalog.schema,
    generation: catalog.generation,
    entries: catalog.entries,
  })));
}
