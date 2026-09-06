import { spawn } from "node:child_process";
import { readFile } from "node:fs/promises";
import { expect, test } from "@playwright/test";
import { reviewAndBirth } from "./creche-test-actions.mjs";
import { downloadArtifact, sha256 } from "./download-artifact.mjs";

const X86 = "conduitos/x86_64/pc";
const AARCH64 = "conduitos/aarch64/virt";
const PROMOTED = Object.freeze([
  Object.freeze({ id: "conduitos/ia32/pc", manifest: "conduitos-ia32-pc-release.json", architecture: "ia32", machine: "pc", firmware: "OVMF_IA32_CODE.fd", bootEntry: "BOOTIA32.EFI", deployment: "conduit-host-conduitos/boot-ia32@1" }),
  Object.freeze({ id: "conduitos/riscv64/virt", manifest: "conduitos-riscv64-virt-release.json", architecture: "riscv64", machine: "virt", firmware: "OpenSBI+U-Boot EFI", bootEntry: "BOOTRISCV64.EFI", deployment: "conduit-host-conduitos/boot-riscv64@1" }),
  Object.freeze({ id: "conduitos/loongarch64/virt", manifest: "conduitos-loongarch64-virt-release.json", architecture: "loongarch64", machine: "virt", firmware: "EDK2 QEMU_EFI.fd", bootEntry: "BOOTLOONGARCH64.EFI", deployment: "conduit-host-conduitos/boot-loongarch64@1" }),
]);
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

async function installRelease(page, name) {
  const root = new URL("../../target/creche-product/artifacts/", import.meta.url);
  const manifest = JSON.parse(await readFile(new URL(name, root), "utf8"));
  const bytes = await readFile(new URL(manifest.artifact.path, root));
  const requested = [];
  await page.route(`**/artifacts/${manifest.artifact.path}`, (route) => { requested.push(new URL(route.request().url()).pathname); return route.fulfill({ status: 200, body: bytes }); });
  await page.route(`**/artifacts/${name}`, (route) => { requested.push(new URL(route.request().url()).pathname); return route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify(manifest) }); });
  return { manifest, requested };
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

test("exact x86_64 product IMAGE obtains and binds as a downloadable spore without device authority", async ({ page }) => {
  const release = await installRelease(page, "conduitos-x86_64-pc-release.json");
  const authority = [];
  await page.exposeFunction("recordConduitOsAuthority", (kind) => authority.push(kind));
  await page.addInitScript(() => {
    navigator.serial = { requestPort: () => globalThis.recordConduitOsAuthority("serial") };
    navigator.usb = { requestDevice: () => globalThis.recordConduitOsAuthority("usb") };
  });
  const runner = await birthBody(page);
  await runner.locator('[data-application-key="physical-target"]').selectOption(X86);
  await expect(runner.locator('[data-application-key="physical-stage-obtain"]')).not.toContainText("waiting");
  let evidence = JSON.parse(await runner.locator("details code").textContent());
  expect(evidence.obtainment).toMatchObject({ target_id: X86, artifact_role: "product-host", image_id: release.manifest.image_id, image_sha256: release.manifest.artifact.sha256, image_bytes: release.manifest.artifact.bytes, does_not_prove: ["load", "boot", "join", "membership"] });
  expect(release.requested).toEqual([
    expect.stringMatching(/\/creche\/artifacts\/conduitos-x86_64-pc-release\.json$/),
    expect.stringMatching(/\/creche\/artifacts\/conduitos-x86_64-pc\.iso$/),
  ]);
  await runner.getByRole("button", { name: "Bind Body invitation" }).click();
  const handoff = runner.locator('[data-application-key="download-spore"]');
  await expect(handoff).toContainText("Download ISO");
  evidence = JSON.parse(await runner.locator("details code").textContent());
  expect(evidence.binding).toMatchObject({
    target_id: X86,
    output: "disk-image",
    fabrication_package_id: "conduitos-image@1",
    deployment_adapter: "conduit-host-conduitos/boot-x86_64@1",
    image_content_digest: release.manifest.artifact.sha256,
    spore_artifact: {
      format: "iso",
      media_type: "application/x-iso9660-image",
      image_content_digest: release.manifest.artifact.sha256,
      image_bytes: release.manifest.artifact.bytes,
      provision_bytes: 4096,
    },
  });
  const downloaded = await downloadArtifact(page, handoff);
  expect(downloaded.filename).toMatch(/-conduitos-native\.iso$/);
  const { readBodyProvisionedMedia } = await import("../../products/creche/browser/creche-native-disk.mjs");
  const nativeIso = {
    isoMagic: new TextDecoder().decode(downloaded.bytes.subarray(32769, 32774)),
    bytes: downloaded.bytes.byteLength,
    contentDigest: await sha256(downloaded.bytes),
    provision: readBodyProvisionedMedia(downloaded.bytes).provision,
  };
  expect(nativeIso).toMatchObject({
    isoMagic: "CD001",
    bytes: release.manifest.artifact.bytes + 4096,
    provision: {
      image_bytes: release.manifest.artifact.bytes,
      spore: { spore_id: evidence.binding.spore_id, body_id: evidence.binding.body_id },
      invitation_provision: { invitation_id: evidence.binding.invitation_id },
    },
  });
  expect(nativeIso.contentDigest).toBe(evidence.binding.spore_artifact.content_digest);
  expect(evidence).toMatchObject({ realization: null, observation: null, admission: null });
  expect(authority).toEqual([]);
  await runner.getByRole("button", { name: "Realize selected Host" }).click();
  await expect(runner.locator("details code")).toContainText('"terminal": "UnavailableWriter"');
  evidence = JSON.parse(await runner.locator("details code").textContent());
  expect(evidence).toMatchObject({ realization: null, observation: null, admission: null });
});

test("catalog exposes every reviewed ConduitOS product Host and keeps carriers exact", async ({ page }) => {
  await installRelease(page, "conduitos-aarch64-virt-release.json");
  const runner = await birthBody(page);
  for (const target of [X86, AARCH64, ...PROMOTED.map(({ id }) => id), "conduitos/armv6/raspberry-pi-model-b-plus-v1.2"]) {
    await expect(runner.locator(`[data-application-key="physical-target"] option[value="${target}"]`)).toHaveCount(1);
  }
  await expect(runner.locator('[data-application-key="physical-target"] option[value="conduitos/generic"]')).toHaveCount(0);
  await runner.locator('[data-application-key="physical-target"]').selectOption(AARCH64);
  await expect(runner.locator('[data-application-key="physical-stage-obtain"]')).not.toContainText("waiting");
  const evidence = JSON.parse(await runner.locator("details code").textContent());
  expect(evidence.target_entry.target_profile).toMatchObject({ architecture: "aarch64", machine: "virt", artifact_role: "product-host", firmware: "QEMU_EFI.fd", boot_entry: "BOOTAA64.EFI", supported_carriers: ["downloadable-disk-image"], unavailable_carriers: ["browser-raw-disk-writer", "browser-vm-launcher", "network-boot"] });
});

for (const target of PROMOTED) {
  test(`${target.architecture} product IMAGE is selectable and Body-bound with exact target truth`, async ({ page }) => {
    const release = await installRelease(page, target.manifest);
    const runner = await birthBody(page);
    await runner.locator('[data-application-key="physical-target"]').selectOption(target.id);
    await expect(runner.locator('[data-application-key="physical-stage-obtain"]')).not.toContainText("waiting");
    const evidence = JSON.parse(await runner.locator("details code").textContent());
    expect(evidence.target_entry.target_profile).toMatchObject({
      architecture: target.architecture,
      machine: target.machine,
      artifact_role: "product-host",
      firmware: target.firmware,
      boot_entry: target.bootEntry,
      supported_carriers: ["downloadable-disk-image"],
    });
    expect(evidence.obtainment).toMatchObject({ target_id: target.id, artifact_role: "product-host" });
    await runner.getByRole("button", { name: "Bind Body invitation" }).click();
    await expect(runner.locator('[data-application-key="physical-stage-bind"]')).not.toContainText("waiting");
    const downloaded = await downloadArtifact(page, runner.locator('[data-application-key="download-spore"]'));
    expect(downloaded.filename).toMatch(/\.iso$/);
    const bound = JSON.parse(await runner.locator("details code").textContent());
    expect(bound.binding).toMatchObject({
      target_id: target.id,
      output: "disk-image",
      deployment_adapter: target.deployment,
      image_content_digest: release.manifest.artifact.sha256,
      spore_artifact: {
        format: "iso",
        image_content_digest: release.manifest.artifact.sha256,
        image_bytes: release.manifest.artifact.bytes,
        provision_bytes: 4096,
      },
    });
  });
}

test("architecture, machine, firmware, bootloader, role, stale IMAGE, and absent loader refusals remain distinct", async ({ page }) => {
  const { manifest } = await installRelease(page, "conduitos-x86_64-pc-release.json");
  await page.goto(entrance.url);
  const terminals = await page.evaluate(async ({ manifest, imageUrl, adapterUrl }) => {
    const image = await import(imageUrl); const adapter = await import(adapterUrl);
    const capture = (work) => { try { work(); return "accepted"; } catch (error) { return error.code; } };
    const mutate = (change) => { const candidate = structuredClone(manifest); change(candidate); return image.validateConduitOsReleaseManifest(candidate, adapter.CONDUITOS_X86_64_PROFILE); };
    const artifactDigest = `sha256:${"9".repeat(64)}`;
    const artifactBytes = manifest.artifact.bytes + 4096;
    const binding = {
      prepared: { image_content_digest: manifest.artifact.sha256 },
      nativeSpore: { content_digest: artifactDigest, bytes: { byteLength: artifactBytes } },
    };
    const loader = {
      loader_id: "fixture-loader",
      target_id: adapter.CONDUITOS_X86_64_PROFILE.target.id,
      machine: adapter.CONDUITOS_X86_64_PROFILE.machine,
      architecture: adapter.CONDUITOS_X86_64_PROFILE.architecture,
      image_content_digest: manifest.artifact.sha256,
      artifact_sha256: artifactDigest,
      artifact_bytes: artifactBytes,
      carrier: "downloadable-disk-image",
      explicit_authority: true,
      load_completed: true,
    };
    return {
      wrongArchitecture: capture(() => mutate((candidate) => { candidate.architecture = "ia32"; })),
      wrongMachine: capture(() => mutate((candidate) => { candidate.machine = "virt"; })),
      missingFirmware: capture(() => mutate((candidate) => { candidate.boot_assets.firmware = ""; })),
      missingBootloader: capture(() => mutate((candidate) => { candidate.boot_assets.boot_entry = ""; })),
      unsupportedRole: capture(() => mutate((candidate) => { candidate.artifact_role = "architecture-proof-appliance"; })),
      staleArtifact: capture(() => mutate((candidate) => { candidate.artifact.sha256 = `sha256:${"0".repeat(64)}`; })),
      unavailableWriter: capture(() => image.validateLoaderEvidence(null, adapter.CONDUITOS_X86_64_PROFILE, binding)),
      acceptedLoader: capture(() => image.validateLoaderEvidence(loader, adapter.CONDUITOS_X86_64_PROFILE, binding)),
    };
  }, { manifest, imageUrl: new URL("targets/conduitos/browser-deployment/image.mjs", entrance.url).href, adapterUrl: new URL("targets/conduitos/browser-deployment/creche-adapter.mjs", entrance.url).href });
  expect(terminals).toEqual({ wrongArchitecture: "WrongArchitecture", wrongMachine: "WrongMachine", missingFirmware: "MissingFirmware", missingBootloader: "MissingBootloader", unsupportedRole: "UnsupportedProductRole", staleArtifact: "StaleArtifact", unavailableWriter: "UnavailableWriter", acceptedLoader: "accepted" });
});
