import { spawn } from "node:child_process";
import { readFile } from "node:fs/promises";
import { expect, test } from "@playwright/test";
import { reviewAndBirth } from "./creche-test-actions.mjs";
import { downloadArtifact, sha256 } from "./download-artifact.mjs";

const TARGET_ID = "avr/avr5/sparkfun-pro-micro-atmega32u4-5v-16mhz";
const MANIFEST_NAME = "avr-promicro-atmega32u4-5v-16mhz.json";
const ARTIFACT_NAME = "promicro-atmega32u4-5v-16mhz.hex";
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
      if (match) {
        clearTimeout(timeout);
        resolve(match[1]);
      }
    };
    child.stdout.on("data", inspect);
    child.stderr.on("data", inspect);
    child.once("exit", (code) => {
      clearTimeout(timeout);
      reject(new Error(`Crèche exited (${code})\n${output}`));
    });
  });
  return { child, url };
}

async function installReviewedRelease(page) {
  const root = new URL("../../target/creche-product/artifacts/", import.meta.url);
  const manifest = JSON.parse(await readFile(new URL(MANIFEST_NAME, root), "utf8"));
  const artifact = await readFile(new URL(ARTIFACT_NAME, root));
  await page.route(`**/artifacts/${MANIFEST_NAME}`, (route) => route.fulfill({
    status: 200,
    contentType: "application/json",
    body: JSON.stringify(manifest),
  }));
  await page.route(`**/artifacts/${ARTIFACT_NAME}`, (route) => route.fulfill({
    status: 200,
    contentType: "application/vnd.conduit.intel-hex",
    body: artifact,
  }));
  return { manifest, artifact };
}

async function birthBody(page) {
  await page.goto(entrance.url);
  await expect(page.locator("#host-state")).toHaveText("Crèche ready");
  await reviewAndBirth(page);
  await page.getByRole("button", { name: "3. Physical Host" }).click();
}

test.beforeEach(async () => {
  entrance = await startCreche();
});

test.afterEach(() => entrance?.child.kill());

test("the exact Pro Micro release becomes a Body-bound downloadable spore", async ({ page }) => {
  const release = await installReviewedRelease(page);
  await birthBody(page);
  const runner = page.locator(".physical-host-runner");
  await runner.locator('[data-application-key="physical-target"]').selectOption(TARGET_ID);
  await expect(runner.locator('[data-application-key="physical-stage-obtain"]')).not.toContainText("waiting");

  let evidence = JSON.parse(await runner.locator("details code").textContent());
  expect(evidence.target_entry).toMatchObject({
    family: { id: "conduit-target-family/sparkfun-pro-micro@1" },
    target: { id: TARGET_ID, profile_id: "atmega32u4-5v-16mhz-caterina-avr109" },
    intentions: [
      { id: "fabricate-new", supported: true },
      { id: "install-existing", supported: false },
      { id: "attach-running", supported: false },
    ],
    carriers: {
      deployment: [{ id: "conduit-carrier/external-programmer-download@1" }],
      installation: [],
      attachment: [],
      observation: [],
    },
    target_profile: {
      schema: "conduit.avr/creche-target-profile@1",
      board: "sparkfun-pro-micro-5v-16mhz",
      fqbn: "SparkFun:avr:promicro:cpu=16MHzatmega32U4",
      mcu: "atmega32u4",
      clock_hz: 16_000_000,
      voltage_mv: 5_000,
      artifact_format: "intel-hex",
      browser_deployment: expect.stringContaining("unavailable"),
      authenticated_join_implemented: false,
    },
  });
  expect(evidence.obtainment).toMatchObject({
    target_id: TARGET_ID,
    image_id: release.manifest.image_id,
    artifact_sha256: release.manifest.artifact.sha256,
    artifact_bytes: release.artifact.byteLength,
    format: "intel-hex",
    browser_deployment_offered: false,
  });

  await runner.getByRole("button", { name: "Bind Body invitation" }).click();
  await expect(runner.locator('[data-application-key="physical-stage-bind"]')).not.toContainText("waiting");
  const download = runner.locator('[data-application-key="download-spore"]');
  evidence = JSON.parse(await runner.locator("details code").textContent());
  expect(evidence.binding).toMatchObject({
    target_id: TARGET_ID,
    output: "intel-hex",
    fabrication_package_id: "conduit-host-avr-promicro@1",
    deployment_adapter: null,
    image_content_digest: release.manifest.artifact.sha256,
    invitation_secret: "embedded in native HEX; redacted",
    spore_artifact: {
      format: "intel-hex",
      image_content_digest: release.manifest.artifact.sha256,
      bootstrap_flash_address: 27_648,
    },
  });
  const downloaded = await downloadArtifact(page, download);
  expect(downloaded.filename).toMatch(/-pro-micro\.hex$/);
  const { readAvrBodySpore } = await import("../../targets/avr/deployment/browser/image.mjs");
  const native = {
    startsWithRecord: new TextDecoder().decode(downloaded.bytes.subarray(0, 1)),
    containsLegacyEnvelope: new TextDecoder().decode(downloaded.bytes).includes("CNDSPOR1"),
    digest: await sha256(downloaded.bytes),
    provision: readAvrBodySpore(downloaded.bytes),
    bytes: downloaded.bytes.byteLength,
  };
  expect(native.startsWithRecord).toBe(":");
  expect(native.containsLegacyEnvelope).toBe(false);
  expect(native.bytes).toBeGreaterThan(release.artifact.byteLength);
  expect(native.digest).toBe(evidence.binding.spore_artifact.content_digest);
  expect(native.provision).toMatchObject({
    protocol: 2,
    spore_id: evidence.binding.spore_id,
    image_id: evidence.binding.image_id,
    invitation_id: evidence.binding.invitation_id,
    body_id: evidence.binding.body_id,
  });

  await runner.getByRole("button", { name: "Realize selected Host" }).click();
  await expect(runner.locator("details code")).toContainText('"terminal": "AbsentProgrammer"');
  evidence = JSON.parse(await runner.locator("details code").textContent());
  expect(evidence).toMatchObject({ realization: null, observation: null, admission: null });
  expect(evidence.terminal).toMatchObject({
    operation: "realize",
    terminal: "AbsentProgrammer",
    authority_requested: false,
    external_work_started: false,
  });
});

test("the Pro Micro contract keeps board, bootloader, reset, port, and flash refusals distinct", async ({ page }) => {
  const { manifest } = await installReviewedRelease(page);
  await birthBody(page);
  const terminals = await page.evaluate(async ({ releaseManifest }) => {
    const module = await import("/creche/targets/avr/browser-deployment/image.mjs");
    const adapter = await import("/creche/targets/avr/browser-deployment/creche-adapter.mjs");
    const capture = (work) => {
      try {
        work();
        return "accepted";
      } catch (error) {
        return error.code;
      }
    };
    const record = (address, type, data = []) => {
      const bytes = [data.length, address >> 8, address & 0xff, type, ...data];
      const checksum = (-bytes.reduce((sum, value) => sum + value, 0)) & 0xff;
      return `:${[...bytes, checksum].map((value) => value.toString(16).padStart(2, "0")).join("").toUpperCase()}`;
    };
    const parse = (lines) => module.parseIntelHex(new TextEncoder().encode(`${lines.join("\n")}\n`));
    const mutate = (change) => {
      const candidate = structuredClone(releaseManifest);
      change(candidate);
      return module.validateProMicroReleaseManifest(candidate, adapter.AVR_PRO_MICRO_PROFILE);
    };
    const nativeDigest = `sha256:${"ab".repeat(32)}`;
    const binding = {
      prepared: { image_content_digest: releaseManifest.artifact.sha256 },
      nativeSpore: { content_id: nativeDigest, programmed_bytes: 1_025, maximum_address: 28_672 },
    };
    const programmer = {
      programmer_id: "fixture-programmer",
      target_id: adapter.AVR_PRO_MICRO_PROFILE.target.id,
      board: adapter.AVR_PRO_MICRO_PROFILE.board,
      mcu: adapter.AVR_PRO_MICRO_PROFILE.mcu,
      bootloader: adapter.AVR_PRO_MICRO_PROFILE.bootloader,
      protocol: adapter.AVR_PRO_MICRO_PROFILE.protocol,
      reset_transition: adapter.AVR_PRO_MICRO_PROFILE.resetTransition,
      reset_observed: true,
      selected_port_generation: 4,
      observed_port_generation: 5,
      artifact_sha256: nativeDigest,
      programmed_bytes: binding.nativeSpore.programmed_bytes,
      maximum_address: binding.nativeSpore.maximum_address,
    };
    return {
      wrongBoard: capture(() => mutate((candidate) => { candidate.board.model = "generic-avr"; })),
      wrongBootloader: capture(() => mutate((candidate) => { candidate.bootloader.protocol = "stk500v1"; })),
      missingReset: capture(() => mutate((candidate) => { candidate.bootloader.reset_transition = null; })),
      wrongFormat: capture(() => mutate((candidate) => { candidate.artifact.format = "binary"; })),
      oversizedImage: capture(() => parse([record(0, 4, [0, 1]), record(0, 0, [1]), record(0, 1)])),
      protectedBootRegion: capture(() => parse([record(0x7000, 0, [1]), record(0, 1)])),
      absentProgrammer: capture(() => module.validateExternalProgrammerEvidence(null, adapter.AVR_PRO_MICRO_PROFILE, binding)),
      programmerWrongBoard: capture(() => module.validateExternalProgrammerEvidence({ ...programmer, board: "other" }, adapter.AVR_PRO_MICRO_PROFILE, binding)),
      programmerWrongBootloader: capture(() => module.validateExternalProgrammerEvidence({ ...programmer, protocol: "stk500v1" }, adapter.AVR_PRO_MICRO_PROFILE, binding)),
      programmerMissingReset: capture(() => module.validateExternalProgrammerEvidence({ ...programmer, reset_observed: false }, adapter.AVR_PRO_MICRO_PROFILE, binding)),
      stalePort: capture(() => module.validateExternalProgrammerEvidence({ ...programmer, observed_port_generation: 4 }, adapter.AVR_PRO_MICRO_PROFILE, binding)),
      staleReleaseImage: capture(() => module.validateExternalProgrammerEvidence({ ...programmer, artifact_sha256: releaseManifest.artifact.sha256 }, adapter.AVR_PRO_MICRO_PROFILE, binding)),
      wrongProgrammedBytes: capture(() => module.validateExternalProgrammerEvidence({ ...programmer, programmed_bytes: 1_024 }, adapter.AVR_PRO_MICRO_PROFILE, binding)),
      wrongMaximumAddress: capture(() => module.validateExternalProgrammerEvidence({ ...programmer, maximum_address: 28_671 }, adapter.AVR_PRO_MICRO_PROFILE, binding)),
      acceptedReceipt: capture(() => module.validateExternalProgrammerEvidence(programmer, adapter.AVR_PRO_MICRO_PROFILE, binding)),
    };
  }, { releaseManifest: manifest });

  expect(terminals).toEqual({
    wrongBoard: "WrongBoard",
    wrongBootloader: "WrongBootloader",
    missingReset: "MissingResetTransition",
    wrongFormat: "WrongArtifactFormat",
    oversizedImage: "OversizedImage",
    protectedBootRegion: "ProtectedBootRegion",
    absentProgrammer: "AbsentProgrammer",
    programmerWrongBoard: "WrongBoard",
    programmerWrongBootloader: "WrongBootloader",
    programmerMissingReset: "MissingResetTransition",
    stalePort: "StalePort",
    staleReleaseImage: "StaleArtifact",
    wrongProgrammedBytes: "StaleArtifact",
    wrongMaximumAddress: "StaleArtifact",
    acceptedReceipt: "accepted",
  });
});
