import assert from "node:assert/strict";
import { chmod, lstat, mkdtemp, mkdir, readFile, readdir, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const root = path.dirname(fileURLToPath(import.meta.url));
const scratch = await mkdtemp(path.join(os.tmpdir(), "conduit-browser-sdk-"));
try {
  const source = path.join(scratch, "bundle");
  const output = path.join(scratch, "package");
  await mkdir(source);
  const runtime = new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]);
  const boot = new TextEncoder().encode("export const admitBrowserBoot = () => {};\n");
  const runtimeFile = await fileIdentity("runtime.wasm", runtime);
  const bootFile = await fileIdentity("browser-boot-profile.mjs", boot);
  const profileId = `sha256:${"1".repeat(64)}`;
  const imageId = `image:sha256:${"2".repeat(64)}`;
  const distributionDigest = await digestFiles([runtimeFile, bootFile]);
  const image = {
    schema: "conduit.browser/bundle-image@1",
    image_id: imageId,
    profile_id: profileId,
    files: [runtimeFile, bootFile],
    boot_module: { role: "profile-gated-boot", path: bootFile.path, sha256: bootFile.sha256 },
    reviewed_distribution: { distribution_digest: distributionDigest, runtime_abi: "conduit.browser/runtime-abi@1" },
  };
  const imageBytes = new TextEncoder().encode(JSON.stringify(image));
  const files = [...image.files, await fileIdentity("conduit-browser-image.json", imageBytes)];
  const distribution = {
    schema: "conduit.release/host-bundle@1",
    target_id: "browser/wasm32/page",
    make_package_id: "browser-wasm@1",
    output: "browser-bundle",
    files: [runtimeFile, bootFile],
    bundle_sha256: distributionDigest,
    reviewed_distribution: { schema: "conduit.browser/reviewed-distribution@1", distribution_id: "browser-reviewed@1", runtime_abi: "conduit.browser/runtime-abi@1" },
  };
  const release = {
    schema: distribution.schema,
    target_id: distribution.target_id,
    make_package_id: distribution.make_package_id,
    output: distribution.output,
    distribution_id: "browser-reviewed@1",
    distribution_sha256: distributionDigest,
    browser_image_id: imageId,
    browser_profile_id: profileId,
    files,
    bundle_sha256: await digestFiles(files),
  };
  await writeFile(path.join(source, "browser-page.json"), JSON.stringify(distribution));
  await writeFile(path.join(source, "browser-bundle-release.json"), JSON.stringify(release));
  await writeFile(path.join(source, "conduit-browser-image.json"), imageBytes);
  await writeFile(path.join(source, "runtime.wasm"), runtime);
  await writeFile(path.join(source, "browser-boot-profile.mjs"), boot);

  const produced = spawnSync(process.execPath, [path.join(root, "package-browser-bundle.mjs"), source, output], { encoding: "utf8" });
  assert.equal(produced.status, 0, produced.stderr);
  const metadata = JSON.parse(await readFile(path.join(output, "bundle/browser-sdk-package.json"), "utf8"));
  assert.equal(metadata.package_id, "@conduit/browser");
  assert.equal(metadata.profile_id, profileId);
  assert.deepEqual(metadata.files, files);
  const packed = spawnSync("npm", ["pack", "--ignore-scripts", "--json"], { cwd: output, encoding: "utf8" });
  assert.equal(packed.status, 0, packed.stderr);
  const listing = JSON.parse(packed.stdout)[0].files.map(({ path: file }) => file);
  assert(listing.includes("browser-sdk.mjs"));
  assert(listing.includes("browser-sdk-plots.mjs"));
  assert(listing.includes("browser-sdk-continuity.mjs"));
  assert(listing.includes("browser-sdk-syntax.mjs"));
  assert(listing.includes("browser-sdk-events.mjs"));
  assert(listing.includes("browser-sdk-face.mjs"));
  assert(listing.includes("browser-sdk-face-actions.mjs"));
  assert(listing.includes("browser-sdk-patchbay.mjs"));
  for (const name of ["owner-face-component.mjs", "owner-face-look.css"]) {
    assert.deepEqual(await readFile(path.join(output, name)), await readFile(path.join(root, "../handbook", name)));
  }
  const renderer = await readFile(path.join(output, "browser-sdk-patchbay.mjs"), "utf8");
  assert(renderer.includes("'./owner-face-component.mjs'"));
  assert(!renderer.includes("'../handbook/"));
  assert(listing.includes("browser-sdk-owner-show-state.mjs"));
  assert(listing.includes("browser-sdk-wardrobe.mjs"));
  assert(listing.includes("host/assets/browser-membership.js"));
  assert(listing.includes("host/assets/browser-host-identity.mjs"));
  assert(listing.includes("host/assets/body-webrtc-sessions.mjs"));
  assert(listing.includes("host/assets/browser-body-host.mjs"));
  assert(listing.includes("host/assets/browser-saved-choice.mjs"));
  assert(listing.includes("host/assets/browser-text-editor.mjs"));
  assert(listing.includes("host/assets/browser-monotonic-timer.mjs"));
  assert(listing.includes("bundle/runtime.wasm"));
  // Import the actual consumer artifact: staging can contain files npm omits.
  const installed = path.join(scratch, "installed");
  await mkdir(installed);
  const archive = JSON.parse(packed.stdout)[0].filename;
  const extracted = spawnSync("tar", ["-xzf", path.join(output, archive), "-C", installed], { encoding: "utf8" });
  assert.equal(extracted.status, 0, extracted.stderr);
  const imported = spawnSync(process.execPath, ["--input-type=module", "-e", "await import('./browser-sdk.mjs')"], { cwd: path.join(installed, "package"), encoding: "utf8" });
  assert.equal(imported.status, 0, imported.stderr);

  // The static application composes this exact bundle, including its SDK graph.
  const app = path.join(scratch, "application");
  await mkdir(app);
  await writeFile(path.join(app, "index.html"), "<!doctype html><html><head><title>Local body</title></head><body></body></html>");
  await writeFile(path.join(app, "navigation.html"), "<nav>Local navigation</nav>");
  await writeFile(path.join(app, "app.mjs"), "export async function startApplication(context) {}\n");
  await writeFile(path.join(app, "birth.json"), JSON.stringify({ name: "My body", initialPlotNames: ["hello"] }));
  const template = {
    schema: "conduit.browser/application-package-template@1", application_id: "example/static-one",
    state_compatibility: { identity: "example/local-state", version: 1 }, host_implementations: ["browser/indexeddb@1"],
    resources: [
      { role: "shell", kind: "content", path: "index.html", maximum_bytes: 1024, dependencies: [] },
      { role: "application-module", kind: "module", path: "app.mjs", maximum_bytes: 1024, dependencies: [] },
      { role: "birth-specification", kind: "content", path: "birth.json", maximum_bytes: 1024, dependencies: [] },
      { role: "navigation", kind: "content", path: "navigation.html", maximum_bytes: 1024, dependencies: [] },
    ],
  };
  const templatePath = path.join(app, "application.template.json");
  await writeFile(templatePath, JSON.stringify(template));
  const staticOutput = path.join(scratch, "static");
  const staticScript = path.resolve(root, "../tools/package-static-application.mjs");
  const staticResult = spawnSync(process.execPath, [staticScript, templatePath, source, staticOutput], { encoding: "utf8" });
  assert.equal(staticResult.status, 0, staticResult.stderr);
  const appManifest = JSON.parse(await readFile(path.join(staticOutput, "application.application.json"), "utf8"));
  assert.equal(appManifest.application_id, template.application_id);
  assert.match(await readFile(path.join(staticOutput, "index.html"), "utf8"), /Content-Security-Policy/);
  assert.equal(await readFile(path.join(staticOutput, "navigation.html"), "utf8"), "<nav>Local navigation</nav>");
  assert.equal(appManifest.resources.filter(resource => resource.role === "runtime").length, 1);
  assert(appManifest.resources.find(resource => resource.role === "browser-sdk").dependencies.length > 0);
  for (const resource of appManifest.resources) {
    assert.equal(await sha256(await readFile(path.join(staticOutput, resource.path))), resource.sha256);
  }
  await writeFile(path.join(app, "index.html"), '<!doctype html><html><head><title>Handbook</title></head><body><!-- conduit-site-navigation --><main>Read</main></body></html>');
  await writeFile(path.join(app, "navigation.html"), '<header class="site-header"><nav aria-label="Main navigation"><a data-section="handbook" href="/conduit/handbook/">Handbook</a></nav></header>');
  const handbookTemplate = {
    ...template,
    application_id: "conduit.application/handbook",
    resources: template.resources.map(resource => resource.role === "navigation"
      ? { ...resource, role: "site-navigation" } : resource),
  };
  await writeFile(templatePath, JSON.stringify(handbookTemplate));
  const handbookOutput = path.join(scratch, "handbook");
  const handbookResult = spawnSync(process.execPath, [staticScript, templatePath, source, handbookOutput], { encoding: "utf8" });
  assert.equal(handbookResult.status, 0, handbookResult.stderr);
  const handbookHtml = await readFile(path.join(handbookOutput, "index.html"), "utf8");
  assert.match(handbookHtml, /<header class="site-header">/);
  assert.match(handbookHtml, /data-section="handbook" aria-current="page"/);
  assert.doesNotMatch(handbookHtml, /conduit-site-navigation/);
  const handbookManifest = JSON.parse(await readFile(path.join(handbookOutput, "application.application.json"), "utf8"));
  assert.equal(handbookManifest.resources.find(resource => resource.path === "index.html").sha256,
    await sha256(new TextEncoder().encode(handbookHtml)));
  await writeFile(templatePath, JSON.stringify(template));
  const overwrite = spawnSync(process.execPath, [staticScript, templatePath, source, staticOutput], { encoding: "utf8" });
  assert.notEqual(overwrite.status, 0);
  template.resources[0].path = "../outside.html";
  await writeFile(templatePath, JSON.stringify(template));
  const unsafe = spawnSync(process.execPath, [staticScript, templatePath, source, path.join(scratch, "unsafe")], { encoding: "utf8" });
  assert.notEqual(unsafe.status, 0);
  assert.match(unsafe.stderr, /Unsafe application path/);

  // These are envelope fixtures, not executable runtime/ABI proof.
  const maximumRuntimeBytes = 20 * 1024 * 1024;
  const atCeiling = new Uint8Array(maximumRuntimeBytes); atCeiling.set(runtime);
  const oversized = new Uint8Array(maximumRuntimeBytes + 1); oversized.set(runtime);
  const wrongMagic = runtime.slice(); wrongMagic[0] = 1;
  const wrongVersion = runtime.slice(); wrongVersion[4] = 2;
  const runtimeCases = [
    { name: "ceiling", bytes: atCeiling, accepted: true },
    { name: "oversized", bytes: oversized, error: /20 MiB/ },
    { name: "truncated", bytes: runtime.slice(0, 7), error: /Wasm.*header/ },
    { name: "magic", bytes: wrongMagic, error: /Wasm.*header/ },
    { name: "version", bytes: wrongVersion, error: /Wasm.*header/ },
    { name: "missing", bytes: null, error: /no.*runtime/ },
    { name: "duplicate", bytes: runtime, duplicate: true, error: /duplicate/ },
  ];
  const runtimeResults = [];
  for (const entry of runtimeCases) {
    const variantSource = path.join(scratch, `runtime-${entry.name}`);
    const variantOutput = path.join(scratch, `runtime-${entry.name}-package`);
    await mkdir(variantSource);
    const variantRuntime = entry.bytes ? await fileIdentity("runtime.wasm", entry.bytes) : null;
    const variantDistributionFiles = variantRuntime ? [variantRuntime, bootFile] : [bootFile];
    const variantDigest = await digestFiles(variantDistributionFiles);
    const variantImage = { ...image, files: variantDistributionFiles,
      reviewed_distribution: { ...image.reviewed_distribution, distribution_digest: variantDigest } };
    const variantImageBytes = new TextEncoder().encode(JSON.stringify(variantImage));
    const variantFiles = [...(entry.duplicate ? [variantRuntime, variantRuntime] : variantDistributionFiles),
      await fileIdentity("conduit-browser-image.json", variantImageBytes)];
    await writeFile(path.join(variantSource, "browser-page.json"), JSON.stringify({ ...distribution,
      files: variantDistributionFiles, bundle_sha256: variantDigest }));
    await writeFile(path.join(variantSource, "browser-bundle-release.json"), JSON.stringify({ ...release,
      distribution_sha256: variantDigest, files: variantFiles, bundle_sha256: await digestFiles(variantFiles) }));
    await writeFile(path.join(variantSource, "conduit-browser-image.json"), variantImageBytes);
    await writeFile(path.join(variantSource, "browser-boot-profile.mjs"), boot);
    if (entry.bytes) await writeFile(path.join(variantSource, "runtime.wasm"), entry.bytes);
    const before = (await readdir(scratch)).sort();
    const result = spawnSync(process.execPath, [path.join(root, "package-browser-bundle.mjs"), variantSource, variantOutput], { encoding: "utf8" });
    runtimeResults.push({ name: entry.name, accepted: result.status === 0 });
    if (result.status !== 0) {
      assert.match(result.stderr, entry.error);
      await assert.rejects(lstat(variantOutput), { code: "ENOENT" });
      assert.deepEqual((await readdir(scratch)).sort(), before);
    } else if (entry.accepted) {
      const bytes = await readFile(path.join(variantOutput, "bundle/runtime.wasm"));
      assert.equal(bytes.length, maximumRuntimeBytes);
      assert.equal(await sha256(bytes), variantRuntime.sha256);
    }
  }
  assert.deepEqual(runtimeResults, runtimeCases.map(entry => ({ name: entry.name, accepted: entry.accepted === true })));

  const moduleResults = [];
  for (const size of [256 * 1024, 256 * 1024 + 1]) {
    const variantSource = path.join(scratch, `module-${size}`);
    const variantOutput = path.join(scratch, `module-${size}-package`);
    await mkdir(variantSource);
    const bytes = new Uint8Array(size).fill(32);
    const variantBoot = await fileIdentity("browser-boot-profile.mjs", bytes);
    const inventory = [runtimeFile, variantBoot];
    const inventoryDigest = await digestFiles(inventory);
    const variantImage = { ...image, files: inventory, boot_module: { ...image.boot_module, sha256: variantBoot.sha256 },
      reviewed_distribution: { ...image.reviewed_distribution, distribution_digest: inventoryDigest } };
    const imageBytes = new TextEncoder().encode(JSON.stringify(variantImage));
    const releaseFiles = [...inventory, await fileIdentity("conduit-browser-image.json", imageBytes)];
    await writeFile(path.join(variantSource, "browser-page.json"), JSON.stringify({ ...distribution,
      files: inventory, bundle_sha256: inventoryDigest }));
    await writeFile(path.join(variantSource, "browser-bundle-release.json"), JSON.stringify({ ...release,
      distribution_sha256: inventoryDigest, files: releaseFiles, bundle_sha256: await digestFiles(releaseFiles) }));
    await writeFile(path.join(variantSource, "conduit-browser-image.json"), imageBytes);
    await writeFile(path.join(variantSource, "runtime.wasm"), runtime);
    await writeFile(path.join(variantSource, "browser-boot-profile.mjs"), bytes);
    const before = (await readdir(scratch)).sort();
    const result = spawnSync(process.execPath, [path.join(root, "package-browser-bundle.mjs"), variantSource, variantOutput], { encoding: "utf8" });
    moduleResults.push({ size, accepted: result.status === 0 });
    if (result.status !== 0) {
      assert.match(result.stderr, /256 KiB/);
      await assert.rejects(lstat(variantOutput), { code: "ENOENT" });
      assert.deepEqual((await readdir(scratch)).sort(), before);
    } else {
      assert.equal(await sha256(await readFile(path.join(variantOutput, "bundle/browser-boot-profile.mjs"))), variantBoot.sha256);
    }
  }
  assert.deepEqual(moduleResults, [{ size: 256 * 1024, accepted: true }, { size: 256 * 1024 + 1, accepted: false }]);

  const metadataCases = ["distribution-schema", "image-abi", "release-package", "release-output",
    "boot-missing", "boot-role", "boot-path", "boot-digest"];
  const metadataResults = [];
  for (const name of metadataCases) {
    const variantSource = path.join(scratch, name);
    const variantOutput = path.join(scratch, `${name}-package`);
    await mkdir(variantSource);
    const variantDistribution = structuredClone(distribution);
    const variantImage = structuredClone(image);
    const variantRelease = structuredClone(release);
    if (name === "distribution-schema") variantDistribution.reviewed_distribution.schema = "unknown@1";
    if (name === "image-abi") variantImage.reviewed_distribution.runtime_abi = "unknown@1";
    if (name === "release-package") variantRelease.make_package_id = "native@1";
    if (name === "release-output") variantRelease.output = "native-bundle";
    if (name === "boot-missing") delete variantImage.boot_module;
    if (name === "boot-role") variantImage.boot_module.role = "ordinary-module";
    if (name === "boot-path") variantImage.boot_module.path = "runtime.wasm";
    if (name === "boot-digest") variantImage.boot_module.sha256 = `sha256:${"f".repeat(64)}`;
    const bytes = new TextEncoder().encode(JSON.stringify(variantImage));
    variantRelease.files = [runtimeFile, bootFile, await fileIdentity("conduit-browser-image.json", bytes)];
    variantRelease.bundle_sha256 = await digestFiles(variantRelease.files);
    await writeFile(path.join(variantSource, "browser-page.json"), JSON.stringify(variantDistribution));
    await writeFile(path.join(variantSource, "browser-bundle-release.json"), JSON.stringify(variantRelease));
    await writeFile(path.join(variantSource, "conduit-browser-image.json"), bytes);
    await writeFile(path.join(variantSource, "runtime.wasm"), runtime);
    await writeFile(path.join(variantSource, "browser-boot-profile.mjs"), boot);
    const before = (await readdir(scratch)).sort();
    const result = spawnSync(process.execPath, [path.join(root, "package-browser-bundle.mjs"), variantSource, variantOutput], { encoding: "utf8" });
    metadataResults.push({ name, accepted: result.status === 0 });
    if (result.status !== 0) {
      assert.match(result.stderr, /exact reviewed BrowserBundle|boot.*declaration/i);
      await assert.rejects(lstat(variantOutput), { code: "ENOENT" });
      assert.deepEqual((await readdir(scratch)).sort(), before);
    }
  }
  assert.deepEqual(metadataResults, metadataCases.map(name => ({ name, accepted: false })));

  const inventoryCases = ["distribution-missing", "distribution-duplicate", "distribution-size", "distribution-order",
    "distribution-digest", "image-size", "image-digest", "image-media-type"];
  const inventoryResults = [];
  for (const name of inventoryCases) {
    const variantSource = path.join(scratch, name);
    const variantOutput = path.join(scratch, `${name}-package`);
    await mkdir(variantSource);
    const variantDistribution = structuredClone(distribution);
    const variantImage = structuredClone(image);
    if (name === "distribution-missing") delete variantDistribution.files;
    if (name === "distribution-duplicate") variantDistribution.files = [runtimeFile, runtimeFile];
    if (name === "distribution-size") variantDistribution.files[0].bytes++;
    if (name === "distribution-order") variantDistribution.files.reverse();
    if (name === "image-size") variantImage.files[0].bytes++;
    if (name === "image-digest") variantImage.files[0].sha256 = `sha256:${"f".repeat(64)}`;
    if (name === "image-media-type") variantImage.files[0].media_type = "text/plain";
    if (name.startsWith("image-")) {
      variantDistribution.files = structuredClone(variantImage.files);
      variantDistribution.bundle_sha256 = await digestFiles(variantDistribution.files);
    }
    if (name === "distribution-digest") variantDistribution.bundle_sha256 = `sha256:${"f".repeat(64)}`;
    variantImage.reviewed_distribution.distribution_digest = variantDistribution.bundle_sha256;
    const variantImageBytes = new TextEncoder().encode(JSON.stringify(variantImage));
    const variantFiles = [runtimeFile, bootFile, await fileIdentity("conduit-browser-image.json", variantImageBytes)];
    await writeFile(path.join(variantSource, "browser-page.json"), JSON.stringify(variantDistribution));
    await writeFile(path.join(variantSource, "browser-bundle-release.json"), JSON.stringify({ ...release,
      distribution_sha256: variantDistribution.bundle_sha256, files: variantFiles, bundle_sha256: await digestFiles(variantFiles) }));
    await writeFile(path.join(variantSource, "conduit-browser-image.json"), variantImageBytes);
    await writeFile(path.join(variantSource, "runtime.wasm"), runtime);
    await writeFile(path.join(variantSource, "browser-boot-profile.mjs"), boot);
    const before = (await readdir(scratch)).sort();
    const result = spawnSync(process.execPath, [path.join(root, "package-browser-bundle.mjs"), variantSource, variantOutput], { encoding: "utf8" });
    inventoryResults.push({ name, accepted: result.status === 0 });
    if (result.status !== 0) {
      assert.match(result.stderr, /inventory|distribution.*digest/i);
      await assert.rejects(lstat(variantOutput), { code: "ENOENT" });
      assert.deepEqual((await readdir(scratch)).sort(), before);
    }
  }
  assert.deepEqual(inventoryResults, inventoryCases.map(name => ({ name, accepted: false })));

  // Refusal after copying the first asset must not publish a partial package.
  const retryOutput = path.join(scratch, "retryable");
  await chmod(path.join(source, "browser-page.json"), 0o600);
  const beforeFailure = (await readdir(scratch)).sort();
  const changedBoot = boot.slice(); changedBoot[0] ^= 1;
  await writeFile(path.join(source, "browser-boot-profile.mjs"), changedBoot);
  const lateRejected = spawnSync(process.execPath, [path.join(root, "package-browser-bundle.mjs"), source, retryOutput], { encoding: "utf8" });
  assert.notEqual(lateRejected.status, 0);
  assert.match(lateRejected.stderr, /failed digest verification/);
  await assert.rejects(lstat(retryOutput), { code: "ENOENT" });
  assert.deepEqual((await readdir(scratch)).sort(), beforeFailure);
  assert.deepEqual(new Uint8Array(await readFile(path.join(source, "runtime.wasm"))), runtime);
  await writeFile(path.join(source, "browser-boot-profile.mjs"), boot);
  const retry = spawnSync(process.execPath, [path.join(root, "package-browser-bundle.mjs"), source, retryOutput], { encoding: "utf8" });
  assert.equal(retry.status, 0, retry.stderr);
  if (process.platform !== "win32") {
    assert.equal((await lstat(retryOutput)).mode & 0o777, 0o777 & ~process.umask());
    assert.equal((await lstat(path.join(retryOutput, "bundle/browser-page.json"))).mode & 0o777, 0o600);
  }
  for (const file of files) {
    const bytes = await readFile(path.join(retryOutput, "bundle", file.path));
    assert.equal(bytes.length, file.bytes);
    assert.equal(await sha256(bytes), file.sha256);
  }
  const retainedMetadata = await readFile(path.join(retryOutput, "bundle/browser-sdk-package.json"));
  const occupied = spawnSync(process.execPath, [path.join(root, "package-browser-bundle.mjs"), source, retryOutput], { encoding: "utf8" });
  assert.notEqual(occupied.status, 0);
  assert.match(occupied.stderr, /already exists/);
  assert.deepEqual(await readFile(path.join(retryOutput, "bundle/browser-sdk-package.json")), retainedMetadata);

  await writeFile(path.join(source, "runtime.wasm"), new Uint8Array([1, 2, 3]));
  const rejected = spawnSync(process.execPath, [path.join(root, "package-browser-bundle.mjs"), source, path.join(scratch, "rejected")], { encoding: "utf8" });
  assert.notEqual(rejected.status, 0);
  assert.match(rejected.stderr, /changed size|failed digest verification/);
} finally {
  await rm(scratch, { recursive: true, force: true });
}

async function fileIdentity(file, bytes) {
  return { path: file, bytes: bytes.byteLength, sha256: await sha256(bytes), media_type: "application/octet-stream" };
}

async function digestFiles(files) {
  return sha256(new TextEncoder().encode(files.map(({ path, sha256 }) => `${path}\0${sha256}\n`).join("").replace(/^/, "conduit.release/host-bundle-content@1\0")));
}

async function sha256(bytes) {
  const value = new Uint8Array(await crypto.subtle.digest("SHA-256", bytes));
  return `sha256:${Array.from(value, (byte) => byte.toString(16).padStart(2, "0")).join("")}`;
}
