import { spawnSync } from "node:child_process";
import { access, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { buildBrowserBundleImage } from "../../targets/browser/deployment/browser/browser-bundle.mjs";

const repository = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const target = path.join(repository, "target");
const distributionRoot = path.join(target, "field-station-distribution");
const stagedDistributionRoot = path.join(target, "workspace-product", "artifacts");
const bundleRoot = path.join(target, "field-station-bundle");
const packageRoot = path.join(target, "field-station-sdk");
const identity = `sha256:${"4".repeat(64)}`;

export async function prepareFieldStationPackage() {
  await Promise.all([bundleRoot, packageRoot].map((directory) =>
    rm(directory, { recursive: true, force: true })));
  const staged = await exists(path.join(stagedDistributionRoot, "browser-page.json"));
  if (!staged) {
    await rm(distributionRoot, { recursive: true, force: true });
    run("cargo", ["+stable", "xtask", "make", "host", "release", "--platform", "browser", "--output",
      distributionRoot, "--source-identity", "field-station-clock@1"]);
  }
  const source = staged ? stagedDistributionRoot : distributionRoot;
  const manifest = JSON.parse(await readFile(path.join(source, "browser-page.json"), "utf8"));
  const distribution = {
    manifest,
    payloads: await Promise.all(manifest.files.map(async (file) => ({
      ...file,
      bytes: new Uint8Array(await readFile(path.join(source, file.path))),
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
  run("node", ["targets/browser/sdk/package-browser-bundle.mjs", bundleRoot, packageRoot]);
}

async function exists(file) {
  try { await access(file); return true; } catch { return false; }
}

function run(command, arguments_) {
  const result = spawnSync(command, arguments_, {
    cwd: repository,
    encoding: "utf8",
    env: { ...process.env, CARGO_TERM_COLOR: "never" },
  });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    throw new Error(`${command} exited ${result.status}\n${result.stderr ?? ""}\n${result.stdout ?? ""}`.trim());
  }
}
