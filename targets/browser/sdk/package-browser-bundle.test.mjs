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
    fabrication_package_id: "browser-wasm@1",
    output: "browser-bundle",
    files: [runtimeFile, bootFile],
    bundle_sha256: distributionDigest,
    reviewed_distribution: { schema: "conduit.browser/reviewed-distribution@1", distribution_id: "browser-reviewed@1", runtime_abi: "conduit.browser/runtime-abi@1" },
  };
  const release = {
    schema: distribution.schema,
    target_id: distribution.target_id,
    fabrication_package_id: distribution.fabrication_package_id,
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
  assert(listing.includes("browser-sdk-forms.mjs"));
  assert(listing.includes("host/assets/browser-body-host.mjs"));
  assert(listing.includes("bundle/runtime.wasm"));
  const imported = spawnSync(process.execPath, ["--input-type=module", "-e", "await import('./browser-sdk.mjs')"], { cwd: output, encoding: "utf8" });
  assert.equal(imported.status, 0, imported.stderr);

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
