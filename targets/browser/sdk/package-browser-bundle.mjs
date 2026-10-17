#!/usr/bin/env node
import { chmod, copyFile, lstat, mkdir, mkdtemp, readFile, rename, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)));
const RELEASE_SCHEMA = "conduit.release/host-bundle@1";
const IMAGE_PATH = "conduit-browser-image.json";
const MAXIMUM_RUNTIME_BYTES = 20 * 1024 * 1024;
const RUNTIME_HEADER = [0, 97, 115, 109, 1, 0, 0, 0];
const encoder = new TextEncoder();
const HOST_ASSETS = path.resolve(ROOT, "../host/assets");

const [bundleArgument, outputArgument] = process.argv.slice(2);
if (!bundleArgument || !outputArgument || process.argv.length !== 4) {
  throw new Error("usage: package-browser-bundle.mjs BROWSER_BUNDLE_DIRECTORY OUTPUT_PACKAGE_DIRECTORY");
}

const bundleRoot = path.resolve(bundleArgument);
const outputRoot = path.resolve(outputArgument);
if (outputRoot === bundleRoot || outputRoot.startsWith(`${bundleRoot}${path.sep}`)) {
  throw new Error("SDK package output must be separate from the source BrowserBundle");
}
try { await lstat(outputRoot); throw new Error("SDK package output already exists"); }
catch (error) { if (error.code !== "ENOENT") throw error; }

const manifests = await Promise.all([
  readJson(path.join(bundleRoot, "browser-page.json")),
  readJson(path.join(bundleRoot, "browser-bundle-release.json")),
  readJson(path.join(bundleRoot, IMAGE_PATH)),
]);
const [distribution, release, image] = manifests.map(manifest => manifest.value);
if (distribution.schema !== RELEASE_SCHEMA || distribution.target_id !== "browser/wasm32/page"
  || distribution.make_package_id !== "browser-wasm@1" || distribution.output !== "browser-bundle"
  || distribution.reviewed_distribution?.runtime_abi !== "conduit.browser/runtime-abi@1"
  || release.schema !== RELEASE_SCHEMA || release.target_id !== distribution.target_id
  || release.distribution_id !== distribution.reviewed_distribution.distribution_id
  || release.distribution_sha256 !== distribution.bundle_sha256
  || image.schema !== "conduit.browser/bundle-image@1"
  || image.image_id !== release.browser_image_id || image.profile_id !== release.browser_profile_id
  || image.reviewed_distribution?.distribution_digest !== distribution.bundle_sha256) {
  throw new Error("SDK package source is not an exact reviewed BrowserBundle for the supported runtime ABI");
}
if (!Array.isArray(release.files) || release.files.length < 1 || release.files.length > 16
  || !Array.isArray(image.files) || image.files.length + 1 !== release.files.length) {
  throw new Error("BrowserBundle package exceeds its finite file bound or has an incomplete IMAGE closure");
}
if (!Array.isArray(distribution.files) || distribution.files.length < 1 || distribution.files.length > 16
  || JSON.stringify(distribution.files) !== JSON.stringify(image.files)) {
  throw new Error("BrowserBundle distribution inventory does not match its IMAGE");
}
if (await digestFiles(distribution.files) !== distribution.bundle_sha256) {
  throw new Error("BrowserBundle distribution content digest is invalid");
}
const imagePaths = new Set([...image.files.map((item) => item.path), IMAGE_PATH]);
const releasePaths = new Set(release.files.map(item => item.path));
if (imagePaths.size !== release.files.length || releasePaths.size !== release.files.length
  || release.files.some((item) => !imagePaths.has(item.path))) {
  throw new Error("BrowserBundle package contains duplicate or undeclared files outside the exact IMAGE");
}
if (image.files.some(file => {
  const released = release.files.find(item => item.path === file.path);
  return !released || released.bytes !== file.bytes || released.sha256 !== file.sha256
    || released.media_type !== file.media_type;
})) {
  throw new Error("BrowserBundle IMAGE inventory does not match its release metadata");
}
if (await digestFiles(release.files) !== release.bundle_sha256) {
  throw new Error("BrowserBundle release content digest is invalid");
}
if (!releasePaths.has("runtime.wasm")) throw new Error("BrowserBundle has no browser runtime");

// The public destination appears only after its complete package is ready.
await mkdir(path.dirname(outputRoot), { recursive: true });
const stageRoot = await mkdtemp(path.join(path.dirname(outputRoot), ".conduit-sdk-stage-"));
try {
  await mkdir(path.join(stageRoot, "bundle"), { recursive: true });
  const packageFiles = [];
  let totalBytes = 0;
  for (const file of release.files) {
    if (!safePath(file.path) || !Number.isSafeInteger(file.bytes) || file.bytes < 1 || file.bytes > 48 * 1024 * 1024
      || !/^sha256:[0-9a-f]{64}$/.test(file.sha256 ?? "")) {
      throw new Error(`BrowserBundle file ${file.path} has invalid bounded identity metadata`);
    }
    if (file.path === "runtime.wasm" && file.bytes > MAXIMUM_RUNTIME_BYTES) {
      throw new Error("BrowserBundle runtime exceeds the browser consumer 20 MiB ceiling");
    }
    const source = path.resolve(bundleRoot, file.path);
    if (!source.startsWith(`${bundleRoot}${path.sep}`)) throw new Error("BrowserBundle path escaped its source directory");
    const metadata = await lstat(source);
    if (!metadata.isFile() || metadata.isSymbolicLink() || metadata.size !== file.bytes) {
      throw new Error(`BrowserBundle asset ${file.path} is absent or changed size`);
    }
    const bytes = file.path === IMAGE_PATH ? manifests[2].bytes : new Uint8Array(await readFile(source));
    if (await sha256(bytes) !== file.sha256) throw new Error(`BrowserBundle asset ${file.path} failed digest verification`);
    if (file.path === "runtime.wasm" && (bytes.byteLength < RUNTIME_HEADER.length
      || RUNTIME_HEADER.some((byte, index) => bytes[index] !== byte))) {
      throw new Error("BrowserBundle runtime has an invalid Wasm version header");
    }
    totalBytes += bytes.byteLength;
    if (totalBytes > 48 * 1024 * 1024) throw new Error("BrowserBundle exceeds the reviewed 48 MiB package bound");
    const destination = path.join(stageRoot, "bundle", file.path);
    await mkdir(path.dirname(destination), { recursive: true });
    await writeFile(destination, bytes);
    await chmod(destination, metadata.mode & 0o777);
    packageFiles.push(file);
  }
  for (const [index, name] of ["browser-page.json", "browser-bundle-release.json"].entries()) {
    const destination = path.join(stageRoot, "bundle", name);
    await writeFile(destination, manifests[index].bytes);
    await chmod(destination, manifests[index].mode);
  }
  for (const name of ["package.json", "browser-sdk.mjs", "browser-sdk-plots.mjs", "browser-sdk-continuity.mjs", "browser-sdk-syntax.mjs", "browser-sdk-events.mjs", "browser-sdk-face.mjs", "browser-sdk-face-actions.mjs", "browser-sdk-wardrobe.mjs", "browser-sdk.d.ts", "README.md"]) {
    await copyFile(path.join(ROOT, name), path.join(stageRoot, name));
  }
  for (const name of ["browser-body-host.mjs", "browser-saved-choice.mjs", "browser-resource-storage.mjs", "browser-monotonic-timer.mjs", "browser-body-input.mjs", "browser-human-input.mjs", "browser-audio-cue.mjs", "browser-pcm-audio.mjs", "browser-plot-effects.mjs", "application-face.mjs", "application-graph-canvas.mjs", "application-syntax-face.mjs", "application-look.mjs", "browser-body-continuity.mjs", "browser-runtime-bridge.mjs", "browser-application-storage.mjs", "browser-membership.js", "browser-host-identity.mjs", "body-webrtc-sessions.mjs", "body-webrtc-session.mjs", "webrtc-datachannel-line.mjs", "webrtc-session-runtime.mjs"]) {
    const source = path.join(HOST_ASSETS, name);
    const destination = path.join(stageRoot, "host", "assets", name);
    await mkdir(path.dirname(destination), { recursive: true });
    await copyFile(source, destination);
  }
  const sdkModule = await readFile(path.join(stageRoot, "browser-sdk.mjs"), "utf8");
  await writeFile(path.join(stageRoot, "browser-sdk.mjs"), sdkModule.replace(
    '"../host/assets/browser-body-host.mjs"', '"./host/assets/browser-body-host.mjs"',
  ).replace(
    '"../host/assets/browser-application-storage.mjs"', '"./host/assets/browser-application-storage.mjs"',
  ).replace(
    '"../host/assets/browser-membership.js"', '"./host/assets/browser-membership.js"',
  ));
  await writeFile(path.join(stageRoot, "bundle", "browser-sdk-package.json"), JSON.stringify({
    schema: "conduit.browser/sdk-package@1",
    package_id: "@conduit/browser",
    package_version: "0.1.0",
    target_id: "browser/wasm32/page",
    runtime_abi: "conduit.browser/runtime-abi@1",
    image_id: image.image_id,
    profile_id: image.profile_id,
    bundle_sha256: release.bundle_sha256,
    files: packageFiles,
  }, null, 2));

  // SDK-owned helpers retain the same packaged Host asset boundary as the facade.
  for (const name of ["browser-sdk-continuity.mjs", "browser-sdk-syntax.mjs"]) {
    const file = path.join(stageRoot, name);
    const source = await readFile(file, "utf8");
    await writeFile(file, source.replaceAll('"../host/assets/', '"./host/assets/').replaceAll("'../host/assets/", "'./host/assets/"));
  }
  // Another producer may have completed while this package was materialized.
  try { await lstat(outputRoot); throw new Error("SDK package output already exists"); }
  catch (error) { if (error.code !== "ENOENT") throw error; }
  await chmod(stageRoot, 0o777 & ~process.umask());
  await rename(stageRoot, outputRoot);
} finally {
  await rm(stageRoot, { recursive: true, force: true });
}

async function readJson(file) {
  const metadata = await lstat(file);
  if (!metadata.isFile() || metadata.size < 1 || metadata.size > 128 * 1024) throw new Error(`${path.basename(file)} exceeds its finite manifest bound`);
  const bytes = await readFile(file);
  return { value: JSON.parse(bytes.toString("utf8")), bytes, mode: metadata.mode & 0o777 };
}
async function digestFiles(files) {
  let source = "conduit.release/host-bundle-content@1\0";
  for (const file of files) source += `${file.path}\0${file.sha256}\n`;
  return sha256(encoder.encode(source));
}
async function sha256(bytes) {
  const value = new Uint8Array(await crypto.subtle.digest("SHA-256", bytes));
  return `sha256:${Array.from(value, (byte) => byte.toString(16).padStart(2, "0")).join("")}`;
}
function safePath(value) {
  return typeof value === "string" && value.length > 0 && value.length <= 256
    && !value.startsWith("/") && !value.includes("\\") && !value.includes("%")
    && !value.split("/").some((segment) => !segment || segment === "." || segment === "..");
}
