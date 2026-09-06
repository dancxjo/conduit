import { spawn } from "node:child_process";
import { readFile } from "node:fs/promises";
import { expect, test } from "@playwright/test";
import { reviewAndBirth } from "./creche-test-actions.mjs";
import { downloadArtifact, sha256 } from "./download-artifact.mjs";

const PI_OS_TARGET = "std/aarch64/raspberry-pi-4-model-b-rev-1.5-4gb";
const BARE_TARGET = "conduitos/armv6/raspberry-pi-model-b-plus-v1.2";
const PI_OS_MANIFEST = "raspios-bookworm-pi4-model-b-rev-1.5-4gb.json";
const BARE_MANIFEST = "rpi-b-plus-image.json";
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

async function installRelease(page, manifestName) {
  const root = new URL("../../target/creche-product/artifacts/", import.meta.url);
  const manifest = JSON.parse(await readFile(new URL(manifestName, root), "utf8"));
  const artifactPaths = Array.isArray(manifest.files) && manifest.files.every((file) => typeof file?.path === "string")
    ? manifest.files.map(({ path }) => path)
    : [manifest.artifact?.path];
  if (artifactPaths.some((path) => typeof path !== "string")) throw new TypeError(`${manifestName} has no downloadable payload`);
  for (const path of artifactPaths) {
    const bytes = await readFile(new URL(path, root));
    await page.route(`**/artifacts/${path}`, (route) => route.fulfill({ status: 200, body: bytes }));
  }
  await page.route(`**/artifacts/${manifestName}`, (route) => route.fulfill({
    status: 200,
    contentType: "application/json",
    body: JSON.stringify(manifest),
  }));
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

test("Raspberry Pi OS is an exact existing-machine package, not a disk image", async ({ page }) => {
  const release = await installRelease(page, PI_OS_MANIFEST);
  const runner = await birthBody(page);
  await runner.locator('[data-application-key="physical-target"]').selectOption(PI_OS_TARGET);
  await expect(runner.locator('[data-application-key="physical-mode"]')).toHaveValue("install-existing");
  await expect(runner.locator('[data-application-key="physical-stage-obtain"]')).not.toContainText("waiting");
  let evidence = JSON.parse(await runner.locator("details code").textContent());
  expect(evidence.target_entry).toMatchObject({
    family: { id: "conduit-target-family/raspberry-pi@1" },
    target: { id: PI_OS_TARGET, model_id: "raspberry-pi/pi-4-model-b-rev-1.5-4gb@1" },
    intentions: [
      { id: "fabricate-new", supported: false },
      { id: "install-existing", supported: true },
      { id: "attach-running", supported: false },
    ],
    target_profile: {
      intention: "install-existing",
      model: "raspberry-pi-4-model-b-rev-1.5-4gb",
      architecture: "aarch64",
      os: "raspberry-pi-os-bookworm-64",
      artifact_format: "native-bundle",
      browser_role: "download Body-bound package only",
    },
  });
  expect(evidence.obtainment).toMatchObject({
    result_kind: "installation",
    target_id: PI_OS_TARGET,
    os: "raspberry-pi-os-bookworm-64",
    architecture: "aarch64",
    package_id: "conduit-host-raspberry-pi@1",
    output: "native-bundle",
    bundle_sha256: release.bundle_sha256,
    does_not_prove: ["body-binding", "installation", "start", "boot-observation", "join", "membership"],
  });

  await runner.getByRole("button", { name: "Bind Body invitation" }).click();
  const handoff = runner.locator('[data-application-key="download-spore"]');
  await expect(handoff).toContainText("Download ZIP");
  evidence = JSON.parse(await runner.locator("details code").textContent());
  expect(evidence.binding).toMatchObject({
    target_id: PI_OS_TARGET,
    output: "native-bundle",
    fabrication_package_id: "conduit-host-raspberry-pi@1",
    deployment_adapter: "conduit-host-raspberry-pi/install-raspios-package@1",
    image_content_digest: release.bundle_sha256,
    spore_artifact: {
      format: "zip",
      media_type: "application/zip",
      image_content_digest: release.bundle_sha256,
      files: expect.arrayContaining([
        expect.objectContaining({ path: "conduit-linux-aarch64", mode: 0o100755 }),
      ]),
    },
  });
  const downloadedPackage = await downloadArtifact(page, handoff);
  expect(downloadedPackage.filename).toMatch(/-raspios-bookworm-64-aarch64\.zip$/);
  const { readBodyBoundZip } = await import("../../products/creche/browser/creche-native-zip.mjs");
  const archive = readBodyBoundZip(downloadedPackage.bytes);
  const nativePackage = {
    magic: new TextDecoder().decode(downloadedPackage.bytes.subarray(0, 4)),
    contentDigest: await sha256(downloadedPackage.bytes),
    files: Array.from(archive.entries.keys()),
    provision: archive.provision,
  };
  expect(nativePackage).toMatchObject({
    magic: "PK\u0003\u0004",
    files: ["conduit-linux-aarch64", "conduit-spore.json"],
    provision: {
      spore: { spore_id: evidence.binding.spore_id, body_id: evidence.binding.body_id },
      invitation_provision: { invitation_id: evidence.binding.invitation_id },
    },
  });
  expect(nativePackage.contentDigest).toBe(evidence.binding.spore_artifact.content_digest);
  await runner.getByRole("button", { name: "Realize selected Host" }).click();
  await expect(runner.locator("details code")).toContainText('"terminal": "UnavailableCredentials"');
  evidence = JSON.parse(await runner.locator("details code").textContent());
  expect(evidence).toMatchObject({ realization: null, observation: null, admission: null });
  expect(evidence.terminal).toMatchObject({
    operation: "realize",
    terminal: "UnavailableCredentials",
    ambient_credentials_used: false,
    ambient_addresses_used: false,
    external_work_started: false,
  });
});

test("bare-metal Model B+ becomes an exact SD spore without browser block authority", async ({ page }) => {
  const release = await installRelease(page, BARE_MANIFEST);
  const runner = await birthBody(page);
  await runner.locator('[data-application-key="physical-target"]').selectOption(BARE_TARGET);
  await expect(runner.locator('[data-application-key="physical-mode"]')).toHaveValue("fabricate-new");
  await expect(runner.locator('[data-application-key="physical-stage-obtain"]')).not.toContainText("waiting");
  let evidence = JSON.parse(await runner.locator("details code").textContent());
  expect(evidence.target_entry).toMatchObject({
    family: { id: "conduit-target-family/raspberry-pi@1" },
    target: { id: BARE_TARGET, profile_id: "bcm2835-armv6-direct-kernel-sd" },
    target_profile: {
      intention: "fabricate-new",
      model: "raspberry-pi-model-b-plus-v1.2",
      architecture: "armv6",
      image_format: "mbr-fat32-sd-image",
      carrier: "removable-sd-card",
      browser_raw_block_authority: false,
      physical_flash_boot_uart_human_gated: true,
    },
  });
  expect(evidence.obtainment).toMatchObject({
    target_id: BARE_TARGET,
    image_id: release.image_id,
    image_sha256: release.artifact.sha256,
    image_bytes: release.artifact.bytes,
    image_format: "mbr-fat32-sd-image",
    browser_raw_block_authority_claimed: false,
    does_not_prove: ["image-write", "boot", "uart-observation", "join", "membership"],
  });
  expect(evidence.obtainment.boot_files.map(({ path }) => path).sort()).toEqual([
    "LICENCE.broadcom", "bootcode.bin", "config.txt", "fixup.dat", "kernel.img", "start.elf",
  ].sort());

  await runner.getByRole("button", { name: "Bind Body invitation" }).click();
  const imageHandoff = runner.locator('[data-application-key="download-spore"]');
  await expect(imageHandoff).toContainText("Download IMG");
  evidence = JSON.parse(await runner.locator("details code").textContent());
  expect(evidence.binding).toMatchObject({
    target_id: BARE_TARGET,
    output: "sd-image",
    fabrication_package_id: "conduit-host-raspberry-pi@1",
    deployment_adapter: "conduit-host-raspberry-pi/flash-removable-media@1",
    image_content_digest: release.artifact.sha256,
    spore_artifact: {
      format: "img",
      media_type: "application/x-raw-disk-image",
      image_content_digest: release.artifact.sha256,
      image_bytes: release.artifact.bytes,
      provision_bytes: 4096,
    },
  });
  const downloadedImage = await downloadArtifact(page, imageHandoff);
  expect(downloadedImage.filename).toMatch(/-conduitos-raspberry-pi-model-b-plus-v1-2\.img$/);
  const { readBodyProvisionedMedia } = await import("../../products/creche/browser/creche-native-disk.mjs");
  const artifact = readBodyProvisionedMedia(downloadedImage.bytes);
  const nativeImage = {
    mbrMagic: Array.from(downloadedImage.bytes.subarray(510, 512)),
    bytes: downloadedImage.bytes.byteLength,
    contentDigest: await sha256(downloadedImage.bytes),
    provision: artifact.provision,
  };
  expect(nativeImage).toMatchObject({
    mbrMagic: [0x55, 0xaa],
    bytes: release.artifact.bytes + 4096,
    provision: {
      image_bytes: release.artifact.bytes,
      spore: { spore_id: evidence.binding.spore_id, body_id: evidence.binding.body_id },
      invitation_provision: { invitation_id: evidence.binding.invitation_id },
    },
  });
  expect(nativeImage.contentDigest).toBe(evidence.binding.spore_artifact.content_digest);
  await runner.getByRole("button", { name: "Realize selected Host" }).click();
  await expect(runner.locator("details code")).toContainText('"terminal": "AbsentWriter"');
  evidence = JSON.parse(await runner.locator("details code").textContent());
  expect(evidence).toMatchObject({ realization: null, observation: null, admission: null });
  expect(evidence.terminal).toMatchObject({
    operation: "realize",
    terminal: "AbsentWriter",
    browser_raw_block_authority_claimed: false,
    external_work_started: false,
  });
});

test("Pi model, architecture, boot partition, image, writer, and unsupported-model refusals stay distinct", async ({ page }) => {
  const release = await installRelease(page, BARE_MANIFEST);
  await page.goto(entrance.url);
  const terminals = await page.evaluate(async ({ releaseManifest, imageUrl, adapterUrl }) => {
    const image = await import(imageUrl);
    const adapter = await import(adapterUrl);
    const capture = (work) => { try { work(); return "accepted"; } catch (error) { return error.code; } };
    const mutate = (change) => { const candidate = structuredClone(releaseManifest); change(candidate); return image.validateRaspberryPiImageManifest(candidate, adapter.RASPBERRY_PI_B_PLUS_PROFILE); };
    const artifactDigest = `sha256:${"9".repeat(64)}`;
    const artifactBytes = releaseManifest.artifact.bytes + 4096;
    const binding = {
      prepared: { image_content_digest: releaseManifest.artifact.sha256 },
      nativeSpore: { content_digest: artifactDigest, bytes: { byteLength: artifactBytes } },
    };
    const writer = {
      writer_id: "fixture-local-writer",
      target_id: adapter.RASPBERRY_PI_B_PLUS_PROFILE.target.id,
      board: adapter.RASPBERRY_PI_B_PLUS_PROFILE.board,
      architecture: adapter.RASPBERRY_PI_B_PLUS_PROFILE.architecture,
      image_content_digest: releaseManifest.artifact.sha256,
      artifact_sha256: artifactDigest,
      artifact_bytes: artifactBytes,
      carrier: "removable-sd-card",
      raw_block_authority: "local-helper-explicit",
      write_completed: true,
      byte_verification_completed: true,
    };
    return {
      wrongModel: capture(() => mutate((candidate) => { candidate.board = "raspberry-pi-5"; })),
      wrongArchitecture: capture(() => mutate((candidate) => { candidate.architecture = "aarch64"; })),
      incompleteBootPartition: capture(() => mutate((candidate) => { candidate.boot_files.pop(); })),
      staleImage: capture(() => mutate((candidate) => { candidate.artifact.sha256 = `sha256:${"0".repeat(64)}`; })),
      absentWriter: capture(() => image.validateImageWriterEvidence(null, adapter.RASPBERRY_PI_B_PLUS_PROFILE, binding)),
      writerWrongModel: capture(() => image.validateImageWriterEvidence({ ...writer, board: "raspberry-pi-zero-v1" }, adapter.RASPBERRY_PI_B_PLUS_PROFILE, binding)),
      writerWrongArchitecture: capture(() => image.validateImageWriterEvidence({ ...writer, architecture: "armv7" }, adapter.RASPBERRY_PI_B_PLUS_PROFILE, binding)),
      writerStaleImage: capture(() => image.validateImageWriterEvidence({ ...writer, artifact_sha256: `sha256:${"1".repeat(64)}` }, adapter.RASPBERRY_PI_B_PLUS_PROFILE, binding)),
      acceptedWriter: capture(() => image.validateImageWriterEvidence(writer, adapter.RASPBERRY_PI_B_PLUS_PROFILE, binding)),
    };
  }, {
    releaseManifest: release,
    imageUrl: new URL("targets/raspberry-pi/deployment/browser/image.mjs", entrance.url).href,
    adapterUrl: new URL("targets/raspberry-pi/deployment/browser/creche-adapter.mjs", entrance.url).href,
  });
  expect(terminals).toEqual({
    wrongModel: "WrongModel",
    wrongArchitecture: "WrongArchitecture",
    incompleteBootPartition: "IncompleteBootPartition",
    staleImage: "StaleImage",
    absentWriter: "AbsentWriter",
    writerWrongModel: "WrongModel",
    writerWrongArchitecture: "WrongArchitecture",
    writerStaleImage: "StaleImage",
    acceptedWriter: "accepted",
  });

  const runner = await birthBody(page);
  await expect(runner.locator('[data-application-key="physical-target"] option[value="std/aarch64/raspberry-pi-5"]')).toHaveCount(0);
  await expect(runner.locator('[data-application-key="physical-target"] option[value="conduitos/armv7/raspberry-pi-3"]')).toHaveCount(0);
});
