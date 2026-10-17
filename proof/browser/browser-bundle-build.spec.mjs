import { expect, test } from "@playwright/test";
import { buildBrowserBundleImage, verifyBrowserBundleImage } from "../../targets/browser/deployment/browser/browser-bundle.mjs";

const encoder = new TextEncoder();
const ids = ["browser/dom-face@1", "browser/dom@1", "browser/indexeddb@1"];

test("checked Browser PROFILE binds reviewed prebuilt bytes into a distinct verifiable IMAGE without a compiler", async () => {
  const distribution = await fixture();
  const minimal = await buildBrowserBundleImage({ checked: checked(ids.slice(0, 2), "1"), distribution });
  const durable = await buildBrowserBundleImage({ checked: checked(ids, "2"), distribution });

  expect(minimal.manifest.distribution_sha256).toBe(distribution.manifest.bundle_sha256);
  expect(durable.manifest.distribution_sha256).toBe(distribution.manifest.bundle_sha256);
  expect(minimal.manifest.browser_build_id).not.toBe(durable.manifest.browser_build_id);
  expect(minimal.manifest.browser_image_id).not.toBe(durable.manifest.browser_image_id);
  expect(minimal.manifest.bundle_sha256).not.toBe(durable.manifest.bundle_sha256);
  expect(minimal.manifest.builder_adapter).toBe("conduit-host-browser/bind-prebuilt@1");
  expect(minimal.payloads.find((item) => item.path === "runtime.wasm").bytes).toEqual(
    durable.payloads.find((item) => item.path === "runtime.wasm").bytes,
  );
  const image = JSON.parse(new TextDecoder().decode(minimal.payloads.find((item) => item.path === "conduit-browser-image.json").bytes));
  expect(image.boot_module).toEqual({
    role: "profile-gated-boot",
    path: "browser-boot-profile.mjs",
    sha256: minimal.payloads.find((item) => item.path === "browser-boot-profile.mjs").sha256,
  });
  expect(await verifyBrowserBundleImage({ checked: checked(ids.slice(0, 2), "1"), release: minimal, distribution })).toMatchObject({
    build_id: minimal.manifest.browser_build_id,
    image_id: minimal.manifest.browser_image_id,
  });
});

test("BrowserBundle BUILD keeps distribution, revisions, dependencies, targets, bounds, and assets as distinct refusals", async () => {
  const cases = [
    [async () => buildBrowserBundleImage({ checked: checked(ids, "a"), distribution: null }), "MissingReviewedDistribution"],
    [async () => buildBrowserBundleImage({ checked: checked([...ids, "browser/missing@1"], "b"), distribution: await fixture() }), "ImplementationAbsent"],
    [async () => buildBrowserBundleImage({ checked: checked(ids, "c"), distribution: await fixture((value) => { value.manifest.reviewed_distribution.implementations[0].revision = 2; }) }), "WrongImplementationRevision"],
    [async () => buildBrowserBundleImage({ checked: checked(ids, "d"), distribution: await fixture((value) => { value.manifest.reviewed_distribution.runtime_abi = "future"; }) }), "IncompatibleRuntimeAbi"],
    [async () => buildBrowserBundleImage({ checked: checked(ids, "e"), distribution: await fixture((value) => { value.manifest.reviewed_distribution.modules[0].dependencies = ["missing.mjs"]; }) }), "ModuleDependencyMissing"],
    [async () => buildBrowserBundleImage({ checked: checked(ids, "f"), distribution: await fixture((value) => { value.manifest.reviewed_distribution.implementations.push({ ...value.manifest.reviewed_distribution.implementations[0] }); }) }), "ConflictingProvider"],
    [async () => buildBrowserBundleImage({ checked: checked(ids, "0"), distribution: await fixture((value) => { value.manifest.reviewed_distribution.targets = ["browser/other/page"]; }) }), "UnsupportedBrowserTarget"],
    [async () => buildBrowserBundleImage({ checked: checked(ids, "1"), distribution: await fixture((value) => { value.manifest.reviewed_distribution.maximum_bundle_bytes = 1; }) }), "ArtifactBound"],
    [async () => buildBrowserBundleImage({ checked: checked(ids, "2"), distribution: await fixture((value) => { value.payloads[0].bytes[0] ^= 1; }) }), "DigestMismatch"],
  ];
  for (const [operation, code] of cases) await expect(operation()).rejects.toMatchObject({ code });

  const distribution = await fixture();
  const release = await buildBrowserBundleImage({ checked: checked(ids, "3"), distribution });
  const widened = { ...release, payloads: [...release.payloads, { path: "surprise.mjs", bytes: encoder.encode("x"), sha256: await sha256(encoder.encode("x")) }] };
  await expect(verifyBrowserBundleImage({ checked: checked(ids, "3"), release: widened, distribution })).rejects.toMatchObject({ code: "UnexpectedAsset" });
});

test("BrowserBundle BUILD refuses cyclic, ambiguous and unbounded module graphs", async () => {
  const cases = [
    ["self cycle", modules => { modules[0].dependencies = [modules[0].path]; }, "ModuleDependencyCycle"],
    ["two-node cycle", modules => { modules[1].dependencies = [modules[0].path]; }, "ModuleDependencyCycle"],
    ["duplicate declaration", modules => { modules.push({ ...modules[0] }); }, "ModuleDeclarationInvalid"],
    ["undeclared dependency", modules => { modules[0].dependencies = ["runtime.wasm"]; }, "ModuleDependencyMissing"],
    ["oversized dependencies", modules => { modules[0].dependencies = Array(17).fill(modules[1].path); }, "ModuleDeclarationInvalid"],
  ];
  const results = [];
  for (const [name, mutate, code] of cases) {
    const distribution = await fixture(value => mutate(value.manifest.reviewed_distribution.modules));
    try {
      await buildBrowserBundleImage({ checked: checked(ids, "a"), distribution });
      results.push({ name, code: "accepted" });
    } catch (error) { results.push({ name, code: error.code }); }
  }
  expect(results).toEqual(cases.map(([name, , code]) => ({ name, code })));

  // A shared dependency is a DAG, not a cycle.
  const shared = await fixture(value => {
    value.manifest.reviewed_distribution.modules[2].dependencies = ["browser-host-bootstrap.mjs"];
  });
  await expect(buildBrowserBundleImage({ checked: checked(ids, "b"), distribution: shared })).resolves.toBeDefined();
});

test("BrowserBundle BUILD binds the complete ordered distribution inventory", async () => {
  const stale = await fixture(value => { value.manifest.bundle_sha256 = `sha256:${"f".repeat(64)}`; });
  const duplicate = await fixture(value => { value.manifest.files.push({ ...value.manifest.files[0] }); });
  const results = [];
  for (const distribution of [stale, duplicate]) {
    try {
      await buildBrowserBundleImage({ checked: checked(ids, "a"), distribution });
      results.push("accepted");
    } catch (error) { results.push(error.code); }
  }
  expect(results).toEqual(["DigestMismatch", "ArtifactBound"]);

  // Payload transport order supplies no new content identity.
  const reordered = await fixture(value => value.payloads.reverse());
  const release = await buildBrowserBundleImage({ checked: checked(ids, "b"), distribution: reordered });
  const image = JSON.parse(new TextDecoder().decode(release.payloads.find(item => item.path === "conduit-browser-image.json").bytes));
  expect(image.files).toEqual(reordered.manifest.files);
  expect(release.manifest.files.slice(0, -1)).toEqual(reordered.manifest.files);
});

test("BrowserBundle verification refuses rehashed IMAGE claims detached from their inputs", async () => {
  const distribution = await fixture();
  const configuration = checked(ids, "a");
  const original = await buildBrowserBundleImage({ checked: configuration, distribution });
  const mutations = [
    image => { image.profile_id = `sha256:${"b".repeat(64)}`; },
    image => { image.source_configuration_id = `sha256:${"b".repeat(64)}`; },
    image => { image.build_id = `build:sha256:${"b".repeat(64)}`; },
    image => { image.reviewed_distribution.source_commit = "commit:forged"; },
    image => { image.reviewed_distribution.distribution_digest = `sha256:${"b".repeat(64)}`; },
    image => { image.files[0].bytes++; },
  ];
  const results = [];
  for (const mutate of mutations) {
    const release = structuredClone(original);
    const binding = release.payloads.find(item => item.path === "conduit-browser-image.json");
    const { image_id, ...image } = JSON.parse(new TextDecoder().decode(binding.bytes));
    mutate(image);
    const imageId = `image:${await sha256(encoder.encode(JSON.stringify(image)))}`;
    binding.bytes = encoder.encode(JSON.stringify({ ...image, image_id: imageId }));
    binding.sha256 = await sha256(binding.bytes);
    release.manifest.browser_image_id = imageId;
    release.manifest.browser_build_id = image.build_id;
    release.manifest.files = release.payloads.map(({ path, bytes, sha256, media_type }) => ({ path, bytes: bytes.byteLength, sha256, media_type }));
    release.manifest.bundle_sha256 = await bundleDigest(release.manifest.files);
    try {
      await verifyBrowserBundleImage({ checked: configuration, release, distribution });
      results.push("accepted");
    } catch (error) { results.push(error.code); }
  }
  expect(results).toEqual(mutations.map(() => "ImageBindingMismatch"));
});

test("BrowserBundle verification binds release declarations and bytes to the reviewed inventory", async () => {
  const distribution = await fixture();
  const configuration = checked(ids, "a");
  const original = await buildBrowserBundleImage({ checked: configuration, distribution });
  const results = [];
  for (const mode of ["runtime", "payload-media", "declared-size", "declared-digest", "missing-inventory"]) {
    const release = structuredClone(original);
    if (mode === "runtime") {
      release.payloads[0].bytes[0] ^= 1;
      release.payloads[0].sha256 = await sha256(release.payloads[0].bytes);
    }
    if (mode === "payload-media") release.payloads[0].media_type = "text/plain";
    release.manifest.files = release.payloads.map(({ path, bytes, sha256, media_type }) => ({ path, bytes: bytes.byteLength, sha256, media_type }));
    release.manifest.bundle_sha256 = await bundleDigest(release.manifest.files);
    if (mode === "declared-size") release.manifest.files[0].bytes++;
    if (mode === "declared-digest") release.manifest.files[0].sha256 = `sha256:${"f".repeat(64)}`;
    if (mode === "missing-inventory") delete release.manifest.files;
    try {
      await verifyBrowserBundleImage({ checked: configuration, release, distribution });
      results.push({ mode, code: "accepted" });
    } catch (error) { results.push({ mode, code: error.code }); }
  }
  expect(results).toEqual(["runtime", "payload-media", "declared-size", "declared-digest", "missing-inventory"]
    .map(mode => ({ mode, code: "ArtifactBindingMismatch" })));
});

function checked(implementations, suffix) {
  return Object.freeze({
    schema: "conduit.creche/checked-browser-configuration@1",
    target_id: "browser/wasm32/page",
    configuration_id: `sha256:${suffix.repeat(64)}`,
    profile_id: `sha256:${suffix.repeat(64)}`,
    selected_implementations: Object.freeze([...implementations].sort()),
  });
}

async function fixture(mutate = () => {}) {
  const sources = new Map([
    ["runtime.wasm", new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0])],
    ["host.mjs", encoder.encode("import './browser-host-bootstrap.mjs';")],
    ["browser-host-bootstrap.mjs", encoder.encode("export const ready = true;")],
    ["browser-boot-profile.mjs", encoder.encode("export const boot = true;")],
  ]);
  const files = [];
  const payloads = [];
  for (const [path, bytes] of sources) {
    const digest = await sha256(bytes);
    const file = { path, bytes: bytes.byteLength, sha256: digest, media_type: path.endsWith(".wasm") ? "application/wasm" : "text/javascript" };
    files.push(file); payloads.push({ ...file, bytes: new Uint8Array(bytes) });
  }
  const value = {
    manifest: {
      schema: "conduit.release/host-bundle@1", target_id: "browser/wasm32/page", make_package_id: "browser-wasm@1",
      output: "browser-bundle", builder_adapter: "conduit-host-browser/build-wasm@1", deployment_adapter: "conduit-host-browser/load@1",
      source_identity: "commit:fixture", bundle_sha256: await bundleDigest(files), files,
      reviewed_distribution: {
        schema: "conduit.browser/reviewed-distribution@1", distribution_id: "conduit.browser/reviewed-distribution@1",
        runtime_abi: "conduit.browser/runtime-abi@1", targets: ["browser/wasm32/page"], toolchain_identity: "rustc:fixture",
        source_commit: "commit:fixture", maximum_bundle_bytes: 1024,
        implementations: ids.map((id) => ({ id, revision: 1, artifact: "browser-runtime-superset.wasm" })),
        modules: [{ path: "host.mjs", dependencies: ["browser-host-bootstrap.mjs"] }, { path: "browser-host-bootstrap.mjs", dependencies: [] }, { path: "browser-boot-profile.mjs", dependencies: [] }],
      },
    },
    payloads,
  };
  mutate(value);
  return value;
}

async function bundleDigest(files) {
  let value = "conduit.release/host-bundle-content@1\0";
  for (const file of files) value += `${file.path}\0${file.sha256}\n`;
  return sha256(encoder.encode(value));
}
async function sha256(bytes) {
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", bytes));
  return `sha256:${Array.from(digest, (byte) => byte.toString(16).padStart(2, "0")).join("")}`;
}
