import { spawn } from "node:child_process";
import { readFile } from "node:fs/promises";
import { expect, test } from "@playwright/test";
import { reviewAndBirth } from "./creche-test-actions.mjs";
import { downloadArtifact } from "./download-artifact.mjs";

const TARGET = "conduitos/aarch64/orange-pi-5-rk3588s";
const MANIFEST = "orange-pi-5-image.json";
let entrance;

async function startCreche() {
  const child = spawn("target/debug/conduit-browser-host", ["--application", "target/creche-product", "--mount", "/creche/", "--no-open"], {
    cwd: new URL("../..", import.meta.url).pathname,
    env: process.env,
    stdio: ["ignore", "pipe", "pipe"],
  });
  let output = "";
  const url = await new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error(`Crèche was not ready\n${output}`)), 10_000);
    const inspect = (chunk) => {
      output += chunk.toString();
      const match = output.match(/CONDUIT_BROWSER_HOST_URL=(http:\/\/127\.0\.0\.1:\d+\/creche\/)/);
      if (match) { clearTimeout(timeout); resolve(match[1]); }
    };
    child.stdout.on("data", inspect); child.stderr.on("data", inspect);
    child.once("exit", (code) => { clearTimeout(timeout); reject(new Error(`Crèche exited (${code})\n${output}`)); });
  });
  return { child, url };
}

async function installRelease(page) {
  const root = new URL("../../target/creche-product/artifacts/", import.meta.url);
  const manifest = JSON.parse(await readFile(new URL(MANIFEST, root), "utf8"));
  const bytes = await readFile(new URL(manifest.artifact.path, root));
  await page.route(`**/artifacts/${manifest.artifact.path}`, (route) => route.fulfill({ status: 200, body: bytes }));
  await page.route(`**/artifacts/${MANIFEST}`, (route) => route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify(manifest) }));
  return manifest;
}

async function birthBody(page) {
  await page.goto(entrance.url);
  await expect(page.locator("#host-state")).toHaveText("Crèche ready");
  await reviewAndBirth(page);
  await page.getByRole("button", { name: "3. Physical Host" }).click();
  return page.locator(".physical-host-runner");
}

test.beforeEach(async () => { entrance = await startCreche(); });
test.afterEach(() => entrance?.child.kill());

test("Orange Pi 5 becomes an exact bare-metal ConduitOS SD spore", async ({ page }) => {
  const release = await installRelease(page);
  const runner = await birthBody(page);
  await runner.locator('[data-application-key="physical-target"]').selectOption(TARGET);
  await expect(runner.locator('[data-application-key="physical-mode"]')).toHaveValue("fabricate-new");
  await expect(runner.locator('[data-application-key="physical-stage-obtain"]')).not.toContainText("waiting");
  let evidence = JSON.parse(await runner.locator("details code").textContent());
  expect(evidence.target_entry).toMatchObject({
    family: { id: "conduit-target-family/orange-pi@1", label: "Orange Pi computers" },
    target: { id: TARGET, model_id: "orange-pi/orange-pi-5@1", profile_id: "conduitos-rk3588s-u-boot-sd" },
    intentions: [
      { id: "fabricate-new", supported: true },
      { id: "install-existing", supported: false },
      { id: "attach-running", supported: false },
    ],
    target_profile: {
      intention: "fabricate-new", model: "orange-pi-5", soc: "rockchip-rk3588s",
      architecture: "aarch64", os: null, image_format: "mbr-rk3588-fat32-sd-image",
      carrier: "removable-microsd-card", browser_raw_block_authority: false,
      physical_flash_boot_uart_human_gated: true,
    },
  });
  expect(evidence.obtainment).toMatchObject({
    target_id: TARGET, image_id: release.image_id, image_sha256: release.artifact.sha256,
    image_bytes: release.artifact.bytes, image_format: "mbr-rk3588-fat32-sd-image",
    browser_raw_block_authority_claimed: false,
    does_not_prove: ["image-write", "boot", "uart-observation", "join", "membership"],
  });
  expect(evidence.obtainment.boot_files.map(({ path }) => path)).toEqual([
    "u-boot-orangepi-5-rk3588s.bin", "Image", "boot.scr",
  ]);

  await runner.getByRole("button", { name: "Bind Body invitation" }).click();
  const handoff = runner.locator('[data-application-key="download-spore"]');
  await expect(handoff).toContainText("Download IMG");
  evidence = JSON.parse(await runner.locator("details code").textContent());
  expect(evidence.binding).toMatchObject({
    target_id: TARGET, output: "sd-image", fabrication_package_id: "conduit-host-orange-pi@1",
    deployment_adapter: "conduit-host-orange-pi/flash-removable-media@1",
    image_content_digest: release.artifact.sha256,
    spore_artifact: { format: "img", media_type: "application/x-raw-disk-image", image_content_digest: release.artifact.sha256, image_bytes: release.artifact.bytes, provision_bytes: 4096 },
  });
  const downloaded = await downloadArtifact(page, handoff);
  expect(downloaded.filename).toMatch(/-conduitos-orange-pi-5\.img$/);
  const { readBodyProvisionedMedia } = await import("../../products/creche/browser/creche-native-disk.mjs");
  const nativeImage = {
    mbrMagic: Array.from(downloaded.bytes.subarray(510, 512)),
    bytes: downloaded.bytes.byteLength,
    provision: readBodyProvisionedMedia(downloaded.bytes).provision,
  };
  expect(nativeImage).toMatchObject({
    mbrMagic: [0x55, 0xaa], bytes: release.artifact.bytes + 4096,
    provision: { image_bytes: release.artifact.bytes, spore: { spore_id: evidence.binding.spore_id, body_id: evidence.binding.body_id } },
  });
  await runner.getByRole("button", { name: "Realize selected Host" }).click();
  await expect(runner.locator("details code")).toContainText('"terminal": "AbsentWriter"');
});

test("architecture, model, boot image, and writer refusals stay distinct", async ({ page }) => {
  const release = await installRelease(page);
  await page.goto(entrance.url);
  const terminals = await page.evaluate(async ({ releaseManifest, imageUrl, adapterUrl }) => {
    const image = await import(imageUrl); const adapter = await import(adapterUrl);
    const capture = (work) => { try { work(); return "accepted"; } catch (error) { return error.code; } };
    const mutate = (change) => { const candidate = structuredClone(releaseManifest); change(candidate); return image.validateOrangePiImageManifest(candidate, adapter.ORANGE_PI_5_PROFILE); };
    const digest = `sha256:${"9".repeat(64)}`; const bytes = releaseManifest.artifact.bytes + 4096;
    const binding = { prepared: { image_content_digest: releaseManifest.artifact.sha256 }, nativeSpore: { content_digest: digest, bytes: { byteLength: bytes } } };
    const writer = { writer_id: "fixture", target_id: adapter.ORANGE_PI_5_PROFILE.target.id, board: "orange-pi-5", machine: "rk3588s", architecture: "aarch64", image_content_digest: releaseManifest.artifact.sha256, artifact_sha256: digest, artifact_bytes: bytes, carrier: "removable-microsd-card", raw_block_authority: "local-helper-explicit", write_completed: true, byte_verification_completed: true };
    return {
      wrongModel: capture(() => mutate((candidate) => { candidate.board = "orange-pi-5b"; })),
      wrongArchitecture: capture(() => mutate((candidate) => { candidate.architecture = "loongarch64"; })),
      incompleteBootImage: capture(() => mutate((candidate) => { candidate.boot_script = null; })),
      staleImage: capture(() => mutate((candidate) => { candidate.artifact.sha256 = `sha256:${"0".repeat(64)}`; })),
      absentWriter: capture(() => image.validateImageWriterEvidence(null, adapter.ORANGE_PI_5_PROFILE, binding)),
      acceptedWriter: capture(() => image.validateImageWriterEvidence(writer, adapter.ORANGE_PI_5_PROFILE, binding)),
    };
  }, { releaseManifest: release, imageUrl: new URL("targets/orange-pi/deployment/browser/image.mjs", entrance.url).href, adapterUrl: new URL("targets/orange-pi/deployment/browser/creche-adapter.mjs", entrance.url).href });
  expect(terminals).toEqual({ wrongModel: "WrongModel", wrongArchitecture: "WrongArchitecture", incompleteBootImage: "IncompleteBootImage", staleImage: "StaleImage", absentWriter: "AbsentWriter", acceptedWriter: "accepted" });
  const runner = await birthBody(page);
  for (const unsupported of ["conduitos/loongarch64/orange-pi-5-rk3588s", "std/aarch64/orange-pi-5-rk3588s", "conduitos/aarch64/orange-pi-5b-rk3588s", "conduitos/aarch64/orange-pi-5-plus-rk3588"]) await expect(runner.locator(`[data-application-key="physical-target"] option[value="${unsupported}"]`)).toHaveCount(0);
});
