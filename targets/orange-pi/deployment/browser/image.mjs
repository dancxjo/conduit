const MAXIMUM_IMAGE_BYTES = 80 * 1024 * 1024;

export async function acquireOrangePiImage(profile, signal) {
  let response;
  try { response = await fetch(profile.manifestPath, { signal, cache: "no-store" }); }
  catch (error) { refuse("ArtifactUnavailable", "reviewed Orange Pi image manifest is unavailable", error); }
  if (!response.ok) refuse("ArtifactUnavailable", `reviewed Orange Pi image manifest returned HTTP ${response.status}`);
  const manifest = await response.json();
  validateOrangePiImageManifest(manifest, profile);
  const artifactResponse = await fetch(new URL(manifest.artifact.path, response.url), { signal, cache: "no-store" });
  if (!artifactResponse.ok) refuse("ArtifactUnavailable", `reviewed Orange Pi SD image returned HTTP ${artifactResponse.status}`);
  const bytes = new Uint8Array(await artifactResponse.arrayBuffer());
  if (bytes.byteLength !== manifest.artifact.bytes || bytes.byteLength < 512 || bytes.byteLength > MAXIMUM_IMAGE_BYTES) refuse("StaleImage", "reviewed Orange Pi image violated its exact finite byte bound");
  if (bytes[510] !== 0x55 || bytes[511] !== 0xaa || bytes[446] !== 0x80 || bytes[450] !== 0x0c) refuse("IncompleteBootImage", "reviewed Orange Pi image omitted its exact bootable MBR/FAT32 partition");
  const digest = await sha256(bytes);
  if (digest !== manifest.artifact.sha256 || digest !== `sha256:${manifest.image_sha256}`) refuse("StaleImage", "reviewed Orange Pi SD image content identity is stale");
  return Object.freeze({ manifest: Object.freeze(manifest), bytes, digest });
}

export function validateOrangePiImageManifest(manifest, profile) {
  if (manifest?.architecture !== profile.architecture) refuse("WrongArchitecture", "reviewed Orange Pi image does not name AArch64");
  if (manifest?.target_id !== profile.target.id || manifest.board !== profile.board || manifest.machine !== profile.machine) refuse("WrongModel", "reviewed image does not name the exact Orange Pi 5 RK3588S substrate");
  const files = [manifest.bootloader_asset, manifest.kernel, manifest.boot_script];
  if (files.some((file) => typeof file?.path !== "string" || !Number.isSafeInteger(file?.bytes) || file.bytes < 1 || !/^sha256:[0-9a-f]{64}$/.test(file?.sha256))) refuse("IncompleteBootImage", "reviewed Orange Pi image manifest omitted exact bootloader, kernel, or boot-script identity");
  if (manifest.schema !== "conduit.conduitos.orange-pi-5-image/v1" || manifest.os !== null
    || manifest.fabrication_package_id !== profile.packageId || manifest.fabrication_package_revision !== 1
    || manifest.output !== "sd-image" || manifest.builder_adapter !== profile.builderAdapter
    || manifest.deployment_adapter !== profile.deploymentAdapter || manifest.boot_mechanism !== profile.bootMechanism
    || manifest.bootloader_start_sector !== 64 || manifest.partition_start_sector !== 32768
    || manifest.partition_scheme !== "mbr/rk3588-loader-at-lba64/single-fat32-lba32768"
    || manifest.artifact?.format !== "mbr-rk3588-fat32-sd-image" || typeof manifest.artifact?.path !== "string" || manifest.artifact.path.includes("/")
    || !Number.isSafeInteger(manifest.artifact?.bytes) || manifest.artifact.bytes < 512 || manifest.artifact.bytes > MAXIMUM_IMAGE_BYTES
    || !/^sha256:[0-9a-f]{64}$/.test(manifest.artifact?.sha256) || manifest.artifact.sha256 !== `sha256:${manifest.image_sha256}`
    || typeof manifest.image_id !== "string" || typeof manifest.source_identity !== "string"
    || manifest.boot_claimed !== false || manifest.physical_proof_claimed !== false) refuse("StaleImage", "reviewed Orange Pi image contract or content identity is stale");
  return Object.freeze({ ...manifest });
}

export function validateImageWriterEvidence(evidence, profile, binding) {
  if (!evidence || typeof evidence.writer_id !== "string") refuse("AbsentWriter", "no explicit local removable-media writer receipt was supplied");
  if (evidence.target_id !== profile.target.id || evidence.board !== profile.board || evidence.machine !== profile.machine) refuse("WrongModel", "local writer receipt does not name the exact Orange Pi 5 RK3588S substrate");
  if (evidence.architecture !== profile.architecture) refuse("WrongArchitecture", "local writer receipt does not name AArch64");
  if (evidence.image_content_digest !== binding.prepared?.image_content_digest || evidence.artifact_sha256 !== binding.nativeSpore?.content_digest || evidence.artifact_bytes !== binding.nativeSpore?.bytes?.byteLength) refuse("StaleImage", "local writer receipt names a different SD image identity");
  if (evidence.carrier !== "removable-microsd-card" || evidence.raw_block_authority !== "local-helper-explicit" || evidence.write_completed !== true || evidence.byte_verification_completed !== true) refuse("WriterEvidenceInvalid", "local writer receipt omitted explicit microSD carrier, authority, write, or byte verification truth");
  return Object.freeze({ ...evidence });
}

async function sha256(bytes) { const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", bytes)); return `sha256:${Array.from(digest, (byte) => byte.toString(16).padStart(2, "0")).join("")}`; }
function refuse(terminal, message, cause) { const error = new Error(message, cause ? { cause } : undefined); error.name = "OrangePiProvisioningRefusal"; error.code = terminal; error.evidence = Object.freeze({ schema: "conduit.orange-pi/creche-provisioning-refusal@1", terminal, message, browser_raw_block_authority_claimed: false, external_work_started: false }); throw error; }
