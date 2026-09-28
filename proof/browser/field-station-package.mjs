import { spawnSync } from "node:child_process";
import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { buildBrowserBundleImage } from "../../targets/browser/deployment/browser/browser-bundle.mjs";

const repository = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const target = path.join(repository, "target");
const distributionRoot = path.join(target, "field-station-distribution");
const bundleRoot = path.join(target, "field-station-bundle");
const packageRoot = path.join(target, "field-station-sdk");
const identity = `sha256:${"4".repeat(64)}`;

export async function prepareFieldStationPackage() {
  await Promise.all([distributionRoot, bundleRoot, packageRoot].map((directory) =>
    rm(directory, { recursive: true, force: true })));
  runCargo(["xtask", "host", "release", "--platform", "browser", "--output", distributionRoot,
    "--source-identity", "field-station-clock@1"]);
  const manifest = JSON.parse(await readFile(path.join(distributionRoot, "browser-page.json"), "utf8"));
  const distribution = {
    manifest,
    payloads: await Promise.all(manifest.files.map(async (file) => ({
      ...file,
      bytes: new Uint8Array(await readFile(path.join(distributionRoot, file.path))),
    }))),
  };
  const release = await buildBrowserBundleImage({
    checked: {
      schema: "conduit.creche/checked-browser-configuration@1",
      target_id: "browser/wasm32/page",
      configuration_id: identity,
      profile_id: identity,
      selected_implementations: ["browser/dom@1", "browser/dom-presentation@1", "browser/indexeddb@1"],
    },
    distribution,
  });
  await mkdir(bundleRoot, { recursive: true });
  await Promise.all(release.payloads.map((payload) =>
    writeFile(path.join(bundleRoot, payload.path), payload.bytes)));
  await Promise.all([
    writeFile(path.join(bundleRoot, "browser-page.json"), JSON.stringify(manifest, null, 2)),
    writeFile(path.join(bundleRoot, "browser-bundle-release.json"), JSON.stringify(release.manifest, null, 2)),
  ]);
  runCargo(["xtask", "host", "browser-sdk-package", "--bundle", bundleRoot, "--output", packageRoot]);
}

function runCargo(arguments_) {
  const result = spawnSync("cargo", ["+stable", ...arguments_], {
    cwd: repository,
    encoding: "utf8",
    env: { ...process.env, CARGO_TERM_COLOR: "never" },
  });
  if (result.status !== 0) throw new Error(`${result.stderr}\n${result.stdout}`.trim());
}
