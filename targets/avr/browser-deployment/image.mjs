const decoder = new TextDecoder("utf-8", { fatal: true });
const FLASH_BYTES = 32_768;
const APPLICATION_BYTES = 28_672;
const BOOT_REGION_START = APPLICATION_BYTES;

export async function acquireProMicroRelease(profile, signal) {
  let response;
  try {
    response = await fetch(profile.manifestPath, { signal, cache: "no-store" });
  } catch (error) {
    refuse("ArtifactUnavailable", "reviewed Pro Micro release manifest is unavailable", error);
  }
  if (!response.ok) refuse("ArtifactUnavailable", `reviewed Pro Micro release manifest returned HTTP ${response.status}`);
  const manifest = await response.json();
  requireManifest(manifest, profile);
  const artifactResponse = await fetch(new URL(manifest.artifact.path, response.url), { signal, cache: "no-store" });
  if (!artifactResponse.ok) refuse("ArtifactUnavailable", `reviewed Pro Micro artifact returned HTTP ${artifactResponse.status}`);
  const bytes = new Uint8Array(await artifactResponse.arrayBuffer());
  if (bytes.byteLength !== manifest.artifact.bytes || bytes.byteLength < 1 || bytes.byteLength > 256 * 1024) {
    refuse("ArtifactBound", "reviewed Pro Micro artifact violated its sealed byte bound");
  }
  const digest = await sha256(bytes);
  if (digest !== manifest.artifact.sha256) refuse("StaleArtifact", "reviewed Pro Micro artifact digest is stale");
  const parsed = parseIntelHex(bytes);
  return Object.freeze({ manifest: Object.freeze(manifest), bytes, digest, parsed });
}

export function parseIntelHex(bytes) {
  let source;
  try {
    source = decoder.decode(bytes);
  } catch (error) {
    refuse("MalformedArtifact", "Pro Micro artifact is not UTF-8 Intel HEX", error);
  }
  let base = 0;
  let eof = false;
  let maximumAddress = 0;
  const programmed = new Set();
  for (const [index, raw] of source.split(/\r?\n/).entries()) {
    const line = raw.trim();
    if (!line) continue;
    if (eof) refuse("MalformedArtifact", "Pro Micro Intel HEX has records after EOF", undefined, { line: index + 1 });
    if (!/^:[0-9A-Fa-f]+$/.test(line) || (line.length - 1) % 2 !== 0) {
      refuse("MalformedArtifact", "Pro Micro Intel HEX record is malformed", undefined, { line: index + 1 });
    }
    const record = Uint8Array.from(line.slice(1).match(/../g), (pair) => Number.parseInt(pair, 16));
    const count = record[0];
    if (record.length !== count + 5 || record.reduce((sum, value) => sum + value, 0) % 256 !== 0) {
      refuse("MalformedArtifact", "Pro Micro Intel HEX length or checksum is invalid", undefined, { line: index + 1 });
    }
    const address = (record[1] << 8) | record[2];
    const type = record[3];
    const data = record.subarray(4, 4 + count);
    if (type === 0) {
      const start = base + address;
      const end = start + count;
      if (end > FLASH_BYTES) refuse("OversizedImage", "Pro Micro Intel HEX addresses bytes beyond ATmega32U4 flash");
      if (end > BOOT_REGION_START) refuse("ProtectedBootRegion", "Pro Micro Intel HEX overlaps the protected Caterina boot region");
      for (let cursor = start; cursor < end; cursor += 1) {
        if (programmed.has(cursor)) refuse("MalformedArtifact", "Pro Micro Intel HEX writes one address more than once");
        programmed.add(cursor);
      }
      maximumAddress = Math.max(maximumAddress, end);
    } else if (type === 1) {
      if (count !== 0 || address !== 0) refuse("MalformedArtifact", "Pro Micro Intel HEX EOF record is invalid");
      eof = true;
    } else if (type === 2) {
      if (count !== 2 || address !== 0) refuse("MalformedArtifact", "Pro Micro Intel HEX segment-address record is invalid");
      base = ((data[0] << 8) | data[1]) << 4;
    } else if (type === 4) {
      if (count !== 2 || address !== 0) refuse("MalformedArtifact", "Pro Micro Intel HEX linear-address record is invalid");
      base = ((data[0] << 8) | data[1]) * 0x1_0000;
    } else if (type !== 3 && type !== 5) {
      refuse("MalformedArtifact", `Pro Micro Intel HEX record type ${type} is unsupported`);
    }
    if (programmed.size > APPLICATION_BYTES) refuse("OversizedImage", "Pro Micro Intel HEX exceeds admitted application flash");
  }
  if (!eof || programmed.size === 0) refuse("MalformedArtifact", "Pro Micro Intel HEX omitted data or EOF");
  return Object.freeze({ programmedBytes: programmed.size, maximumAddress });
}

export function validateExternalProgrammerEvidence(evidence, profile, binding) {
  if (!evidence || typeof evidence.programmer_id !== "string") {
    refuse("AbsentProgrammer", "no explicit external Pro Micro programmer receipt was supplied");
  }
  if (evidence.target_id !== profile.target.id || evidence.board !== profile.board || evidence.mcu !== profile.mcu) {
    refuse("WrongBoard", "external programmer receipt does not name the exact Pro Micro board and ATmega32U4");
  }
  if (evidence.bootloader !== profile.bootloader || evidence.protocol !== profile.protocol) {
    refuse("WrongBootloader", "external programmer receipt does not name Caterina over AVR109");
  }
  if (evidence.reset_transition !== profile.resetTransition || evidence.reset_observed !== true) {
    refuse("MissingResetTransition", "external programmer receipt omitted the exact 1200-baud reset transition");
  }
  if (!Number.isSafeInteger(evidence.selected_port_generation)
    || !Number.isSafeInteger(evidence.observed_port_generation)
    || evidence.observed_port_generation <= evidence.selected_port_generation) {
    refuse("StalePort", "external programmer receipt did not observe a fresh post-reset port generation");
  }
  if (evidence.artifact_sha256 !== binding.prepared.image_content_digest) {
    refuse("StaleArtifact", "external programmer receipt names a different artifact digest");
  }
  if (!Number.isSafeInteger(evidence.programmed_bytes) || evidence.programmed_bytes > APPLICATION_BYTES) {
    refuse("OversizedImage", "external programmer receipt exceeds admitted application flash");
  }
  if (!Number.isSafeInteger(evidence.maximum_address) || evidence.maximum_address > BOOT_REGION_START) {
    refuse("ProtectedBootRegion", "external programmer receipt overlaps the protected Caterina boot region");
  }
  return Object.freeze({ ...evidence });
}

export function validateProMicroReleaseManifest(manifest, profile) {
  requireManifest(manifest, profile);
  return Object.freeze({ ...manifest });
}

function requireManifest(manifest, profile) {
  if (manifest?.schema !== "conduit.release/avr-intel-hex@1"
    || manifest.fabrication_package_id !== profile.packageId
    || manifest.fabrication_package_revision !== 1
    || manifest.output !== "intel-hex" || manifest.builder_adapter !== profile.builderAdapter
    || manifest.deployment_adapter !== null || typeof manifest.image_id !== "string"
    || typeof manifest.source_identity !== "string") {
    refuse("StaleArtifact", "reviewed Pro Micro release contract is stale");
  }
  if (manifest.target_id !== profile.target.id || manifest.board?.model !== profile.board
    || manifest.board?.fqbn !== profile.fqbn || manifest.board?.mcu !== profile.mcu
    || manifest.board?.clock_hz !== profile.clockHz || manifest.board?.voltage_mv !== 5_000) {
    refuse("WrongBoard", "reviewed release does not match the exact Pro Micro board profile");
  }
  if (manifest.bootloader?.name !== profile.bootloader || manifest.bootloader?.protocol !== profile.protocol) {
    refuse("WrongBootloader", "reviewed release does not match Caterina over AVR109");
  }
  if (manifest.bootloader?.reset_transition !== profile.resetTransition
    || manifest.bootloader?.browser_deployment_implemented !== false) {
    refuse("MissingResetTransition", "reviewed release does not retain the exact external reset contract");
  }
  if (manifest.flash?.total_bytes !== FLASH_BYTES || manifest.flash?.application_bytes !== APPLICATION_BYTES
    || manifest.flash?.boot_region_start !== BOOT_REGION_START || manifest.flash?.boot_region_bytes !== 4_096) {
    refuse("FlashBounds", "reviewed release does not retain exact ATmega32U4 flash bounds");
  }
  if (manifest.artifact?.format !== "intel-hex" || typeof manifest.artifact?.path !== "string"
    || manifest.artifact.path.includes("/") || !Number.isSafeInteger(manifest.artifact.bytes)
    || !/^sha256:[0-9a-f]{64}$/.test(manifest.artifact.sha256)) {
    refuse("WrongArtifactFormat", "reviewed Pro Micro release is not one bounded Intel HEX artifact");
  }
}

async function sha256(bytes) {
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", bytes));
  return `sha256:${Array.from(digest, (byte) => byte.toString(16).padStart(2, "0")).join("")}`;
}

function refuse(terminal, message, cause, details = {}) {
  const error = new Error(message, cause ? { cause } : undefined);
  error.name = "AvrProMicroRefusal";
  error.code = terminal;
  error.evidence = Object.freeze({
    schema: "conduit.avr/creche-provisioning-refusal@1",
    terminal,
    message,
    authority_requested: false,
    external_work_started: false,
    ...details,
  });
  throw error;
}
