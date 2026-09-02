const REQUIRED_BOOT_FILES = Object.freeze([
  "LICENCE.broadcom",
  "bootcode.bin",
  "config.txt",
  "fixup.dat",
  "kernel.img",
  "start.elf",
]);
const MAXIMUM_IMAGE_BYTES = 80 * 1024 * 1024;

export async function acquireRaspberryPiImage(profile, signal) {
  let response;
  try {
    response = await fetch(profile.manifestPath, { signal, cache: "no-store" });
  } catch (error) {
    refuse("ArtifactUnavailable", "reviewed Raspberry Pi image manifest is unavailable", error);
  }
  if (!response.ok) refuse("ArtifactUnavailable", `reviewed Raspberry Pi image manifest returned HTTP ${response.status}`);
  const manifest = await response.json();
  validateRaspberryPiImageManifest(manifest, profile);
  const artifactResponse = await fetch(new URL(manifest.artifact.path, response.url), { signal, cache: "no-store" });
  if (!artifactResponse.ok) refuse("ArtifactUnavailable", `reviewed Raspberry Pi SD image returned HTTP ${artifactResponse.status}`);
  const bytes = new Uint8Array(await artifactResponse.arrayBuffer());
  if (bytes.byteLength !== manifest.artifact.bytes || bytes.byteLength < 512 || bytes.byteLength > MAXIMUM_IMAGE_BYTES) {
    refuse("StaleImage", "reviewed Raspberry Pi image violated its exact finite byte bound");
  }
  if (bytes[510] !== 0x55 || bytes[511] !== 0xaa || bytes[450] !== 0x0c) {
    refuse("IncompleteBootPartition", "reviewed Raspberry Pi image omitted its exact MBR/FAT32 boot partition");
  }
  const digest = await sha256(bytes);
  if (digest !== manifest.artifact.sha256 || digest !== `sha256:${manifest.image_sha256}`) {
    refuse("StaleImage", "reviewed Raspberry Pi SD image content identity is stale");
  }
  return Object.freeze({ manifest: Object.freeze(manifest), bytes, digest });
}

export function validateRaspberryPiImageManifest(manifest, profile) {
  if (manifest?.architecture !== profile.architecture) {
    refuse("WrongArchitecture", "reviewed Raspberry Pi image does not name the selected ARMv6 architecture");
  }
  if (manifest?.target_id !== profile.target.id || manifest.board !== profile.board || manifest.machine !== profile.machine) {
    refuse("WrongModel", "reviewed Raspberry Pi image does not name the selected exact Raspberry Pi substrate");
  }
  const files = Array.isArray(manifest.boot_files) ? manifest.boot_files : [];
  const paths = new Set(files.map((file) => file?.path));
  if (files.length !== REQUIRED_BOOT_FILES.length || REQUIRED_BOOT_FILES.some((path) => !paths.has(path))
    || files.some((file) => !Number.isSafeInteger(file?.bytes) || file.bytes < 1 || !/^sha256:[0-9a-f]{64}$/.test(file?.sha256))) {
    refuse("IncompleteBootPartition", "reviewed Raspberry Pi image manifest omitted an exact verified boot file");
  }
  if (manifest.schema !== "conduit.conduitos.armv6-rpi-image/v1"
    || manifest.fabrication_package_id !== profile.packageId
    || manifest.fabrication_package_revision !== 2
    || manifest.output !== "sd-image"
    || manifest.builder_adapter !== profile.builderAdapter
    || manifest.deployment_adapter !== profile.deploymentAdapter
    || manifest.boot_mechanism !== profile.bootMechanism
    || manifest.partition_scheme !== "mbr/single-fat32-lba"
    || manifest.files?.length !== REQUIRED_BOOT_FILES.length
    || manifest.artifact?.format !== "mbr-fat32-sd-image"
    || typeof manifest.artifact?.path !== "string" || manifest.artifact.path.includes("/")
    || !Number.isSafeInteger(manifest.artifact?.bytes) || manifest.artifact.bytes < 512 || manifest.artifact.bytes > MAXIMUM_IMAGE_BYTES
    || !/^sha256:[0-9a-f]{64}$/.test(manifest.artifact?.sha256)
    || manifest.artifact.sha256 !== `sha256:${manifest.image_sha256}`
    || typeof manifest.image_id !== "string" || typeof manifest.source_identity !== "string"
    || manifest.boot_claimed !== false || manifest.physical_proof_claimed !== false) {
    refuse("StaleImage", "reviewed Raspberry Pi image contract or content identity is stale");
  }
  return Object.freeze({ ...manifest });
}

export function validateImageWriterEvidence(evidence, profile, binding) {
  if (!evidence || typeof evidence.writer_id !== "string") {
    refuse("AbsentWriter", "no explicit local removable-media writer receipt was supplied");
  }
  if (evidence.target_id !== profile.target.id || evidence.board !== profile.board) {
    refuse("WrongModel", "local writer receipt does not name the exact Model B+ v1.2 substrate");
  }
  if (evidence.architecture !== profile.architecture) {
    refuse("WrongArchitecture", "local writer receipt does not name ARMv6");
  }
  if (evidence.image_content_digest !== binding.prepared?.image_content_digest
    || evidence.artifact_sha256 !== binding.nativeSpore?.content_digest
    || evidence.artifact_bytes !== binding.nativeSpore?.bytes?.byteLength) {
    refuse("StaleImage", "local writer receipt names a different SD image identity");
  }
  if (evidence.carrier !== "removable-sd-card" || evidence.raw_block_authority !== "local-helper-explicit"
    || evidence.write_completed !== true || evidence.byte_verification_completed !== true) {
    refuse("WriterEvidenceInvalid", "local writer receipt omitted explicit SD carrier, authority, write, or byte verification truth");
  }
  return Object.freeze({ ...evidence });
}

async function sha256(bytes) {
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", bytes));
  return `sha256:${Array.from(digest, (byte) => byte.toString(16).padStart(2, "0")).join("")}`;
}

function refuse(terminal, message, cause) {
  const error = new Error(message, cause ? { cause } : undefined);
  error.name = "RaspberryPiProvisioningRefusal";
  error.code = terminal;
  error.evidence = Object.freeze({
    schema: "conduit.raspberry-pi/creche-provisioning-refusal@1",
    terminal,
    message,
    browser_raw_block_authority_claimed: false,
    external_work_started: false,
  });
  throw error;
}
