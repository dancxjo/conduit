import assert from "node:assert/strict";
import test from "node:test";
import { acquireRaspberryPiImage } from "../../targets/raspberry-pi/deployment/browser/image.mjs";
import { acquireOrangePiImage } from "../../targets/orange-pi/deployment/browser/image.mjs";

const encoder = new TextEncoder();
const RASPBERRY_PI_B_PLUS_PROFILE = Object.freeze({
  target: { id: "conduitos/armv6/raspberry-pi-model-b-plus-v1.2" },
  architecture: "armv6", board: "raspberry-pi-model-b-plus-v1.2", machine: "BCM2835/ARM1176JZF-S",
  packageId: "conduit-host-raspberry-pi@1", builderAdapter: "conduit-host-raspberry-pi/build-sd-image@1",
  deploymentAdapter: "conduit-host-raspberry-pi/flash-removable-media@1",
  bootMechanism: "raspberry-pi-videocore-firmware-direct-kernel",
});
const ORANGE_PI_5_PROFILE = Object.freeze({
  target: { id: "conduitos/aarch64/orange-pi-5-rk3588s" },
  architecture: "aarch64", board: "orange-pi-5", machine: "rk3588s",
  packageId: "conduit-host-orange-pi@1", builderAdapter: "conduit-host-orange-pi/build-conduitos-sd-image@1",
  deploymentAdapter: "conduit-host-orange-pi/flash-removable-media@1",
  bootMechanism: "rk3588s-bootrom-u-boot-booti-conduitos-image",
});

test("Raspberry Pi obtains the exact catalog-resolved image", async () => {
  const bytes = bootableImage();
  const digest = await sha256(bytes);
  const bootFiles = ["LICENCE.broadcom", "bootcode.bin", "config.txt", "fixup.dat", "kernel.img", "start.elf"]
    .map((path) => ({ path, bytes: 1, sha256: `sha256:${"1".repeat(64)}` }));
  const manifest = {
    schema: "conduit.conduitos.armv6-rpi-image/v1",
    architecture: RASPBERRY_PI_B_PLUS_PROFILE.architecture,
    target_id: RASPBERRY_PI_B_PLUS_PROFILE.target.id,
    board: RASPBERRY_PI_B_PLUS_PROFILE.board,
    machine: RASPBERRY_PI_B_PLUS_PROFILE.machine,
    fabrication_package_id: RASPBERRY_PI_B_PLUS_PROFILE.packageId,
    fabrication_package_revision: 2,
    output: "sd-image",
    builder_adapter: RASPBERRY_PI_B_PLUS_PROFILE.builderAdapter,
    deployment_adapter: RASPBERRY_PI_B_PLUS_PROFILE.deploymentAdapter,
    boot_mechanism: RASPBERRY_PI_B_PLUS_PROFILE.bootMechanism,
    partition_scheme: "mbr/single-fat32-lba",
    boot_files: bootFiles,
    files: bootFiles,
    artifact: { path: "rpi.img", format: "mbr-fat32-sd-image", bytes: bytes.byteLength, sha256: digest },
    image_sha256: digest.slice(7), image_id: `image:${digest}`, source_identity: "git:reviewed",
    boot_claimed: false, physical_proof_claimed: false,
  };
  let acquired;
  const release = await acquireRaspberryPiImage(RASPBERRY_PI_B_PLUS_PROFILE, undefined, {
    resolved: {
      manifest: encoder.encode(JSON.stringify(manifest)),
      async acquire(artifact, maximumBytes) { acquired = { artifact, maximumBytes }; return bytes; },
    },
  });
  assert.equal(release.digest, digest);
  assert.equal(acquired.artifact.path, "rpi.img");
  assert.equal(acquired.maximumBytes, 80 * 1024 * 1024);
});

test("Orange Pi validates catalog manifest identity before artifact acquisition", async () => {
  const bytes = bootableImage({ active: true });
  const digest = await sha256(bytes);
  const file = (path) => ({ path, bytes: 1, sha256: `sha256:${"2".repeat(64)}` });
  const manifest = {
    schema: "conduit.conduitos.orange-pi-5-image/v1", os: null,
    architecture: ORANGE_PI_5_PROFILE.architecture,
    target_id: ORANGE_PI_5_PROFILE.target.id, board: ORANGE_PI_5_PROFILE.board, machine: ORANGE_PI_5_PROFILE.machine,
    fabrication_package_id: ORANGE_PI_5_PROFILE.packageId, fabrication_package_revision: 1,
    output: "sd-image", builder_adapter: ORANGE_PI_5_PROFILE.builderAdapter,
    deployment_adapter: ORANGE_PI_5_PROFILE.deploymentAdapter, boot_mechanism: ORANGE_PI_5_PROFILE.bootMechanism,
    bootloader_start_sector: 64, partition_start_sector: 32768,
    partition_scheme: "mbr/rk3588-loader-at-lba64/single-fat32-lba32768",
    bootloader_asset: file("idbloader.img"), kernel: file("kernel.img"), boot_script: file("boot.scr"),
    artifact: { path: "orange.img", format: "mbr-rk3588-fat32-sd-image", bytes: bytes.byteLength, sha256: digest },
    image_sha256: digest.slice(7), image_id: `image:${digest}`, source_identity: "git:reviewed",
    boot_claimed: false, physical_proof_claimed: false,
  };
  let acquireCalls = 0;
  const resolved = {
    manifest: encoder.encode(JSON.stringify({ ...manifest, target_id: "wrong-target" })),
    async acquire() { acquireCalls += 1; return bytes; },
  };
  await assert.rejects(
    acquireOrangePiImage(ORANGE_PI_5_PROFILE, undefined, { resolved }),
    (error) => error.code === "WrongModel",
  );
  assert.equal(acquireCalls, 0);
  resolved.manifest = encoder.encode(JSON.stringify(manifest));
  const release = await acquireOrangePiImage(ORANGE_PI_5_PROFILE, undefined, { resolved });
  assert.equal(release.digest, digest);
  assert.equal(acquireCalls, 1);
});

function bootableImage({ active = false } = {}) {
  const bytes = new Uint8Array(512);
  bytes[446] = active ? 0x80 : 0;
  bytes[450] = 0x0c;
  bytes[510] = 0x55;
  bytes[511] = 0xaa;
  return bytes;
}

async function sha256(bytes) {
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", bytes));
  return `sha256:${Array.from(digest, (byte) => byte.toString(16).padStart(2, "0")).join("")}`;
}
