import assert from "node:assert/strict";
import { mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
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
  const packed = spawnSync("npm", ["pack", "--dry-run", "--json"], { cwd: output, encoding: "utf8" });
  assert.equal(packed.status, 0, packed.stderr);
  const listing = JSON.parse(packed.stdout)[0].files.map(({ path: file }) => file);
  assert(listing.includes("browser-sdk.mjs"));
  assert(listing.includes("browser-sdk-plots.mjs"));
  assert(listing.includes("browser-sdk-continuity.mjs"));
  assert(listing.includes("browser-sdk-syntax.mjs"));
  assert(listing.includes("browser-sdk-events.mjs"));
  assert(listing.includes("browser-sdk-face.mjs"));
  assert(listing.includes("host/assets/browser-membership.js"));
  assert(listing.includes("host/assets/browser-host-identity.mjs"));
  assert(listing.includes("host/assets/body-webrtc-sessions.mjs"));
  assert(listing.includes("host/assets/browser-body-host.mjs"));
  assert(listing.includes("bundle/runtime.wasm"));
  const imported = spawnSync(process.execPath, ["--input-type=module", "-e", "await import('./browser-sdk.mjs')"], { cwd: output, encoding: "utf8" });
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
