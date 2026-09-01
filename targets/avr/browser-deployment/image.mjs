const decoder = new TextDecoder("utf-8", { fatal: true });
const encoder = new TextEncoder();
const FLASH_BYTES = 32_768;
const APPLICATION_BYTES = 28_672;
const BOOT_REGION_START = APPLICATION_BYTES;
export const AVR_SPORE_REGION = Object.freeze({ start: 27_648, bytes: 1_024 });
const SPORE_MAGIC = encoder.encode("CONDUIT_SPORE@1\0");
const SPORE_VERSION = 1;
const SPORE_FIXED_BYTES = SPORE_MAGIC.byteLength + 1 + 2 + 8 + 32 + 32;
const MAX_ID_BYTES = 128;
const SPORE_FIELD_NAMES = Object.freeze(["spore_id", "image_id", "invitation_id", "body_id"]);

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

export async function bindAvrBodySpore(imageBytes, prepared, cryptoApi = globalThis.crypto) {
  if (prepared?.output !== "intel-hex"
    || prepared?.target_id !== "avr/avr5/sparkfun-pro-micro-atmega32u4-5v-16mhz") {
    refuse("SporeTarget", "prepared Body binding is not for the exact Pro Micro target");
  }
  const base = imageBytes instanceof Uint8Array ? imageBytes : new Uint8Array(imageBytes);
  const parsedImage = parseIntelHex(base);
  if (parsedImage.maximumAddress > AVR_SPORE_REGION.start) {
    refuse("SporeOverlap", "generic Pro Micro IMAGE overlaps the reserved native Spore region");
  }
  const provision = encodeProvision(prepared);
  const lines = recordsOf(base).filter((item) => item.type !== 1).map((item) => item.line);
  lines.push(intelHexRecord(0, 4, Uint8Array.from([0, 0])));
  for (let offset = 0; offset < provision.bytes.byteLength; offset += 16) {
    lines.push(intelHexRecord(AVR_SPORE_REGION.start + offset, 0, provision.bytes.subarray(offset, offset + 16)));
  }
  lines.push(intelHexRecord(0, 1));
  const bytes = encoder.encode(`${lines.join("\n")}\n`);
  const parsed = parseIntelHex(bytes);
  const recovered = readAvrBodySpore(bytes);
  for (const name of SPORE_FIELD_NAMES) {
    if (recovered[name] !== prepared[name]) refuse("SporeBinding", `native AVR Spore lost ${name}`);
  }
  const contentId = await sha256With(bytes, cryptoApi);
  return Object.freeze({
    schema: "conduit.avr/native-body-spore@1",
    format: "intel-hex",
    bytes,
    content_id: contentId,
    image_content_id: prepared.image_content_digest,
    spore_id: prepared.spore_id,
    programmed_bytes: parsed.programmedBytes,
    maximum_address: parsed.maximumAddress,
    bootstrap_bytes: provision.length,
    bootstrap_flash_address: AVR_SPORE_REGION.start,
  });
}

export function readAvrBodySpore(value) {
  const bytes = value instanceof Uint8Array ? value : new Uint8Array(value);
  parseIntelHex(bytes);
  const region = new Uint8Array(AVR_SPORE_REGION.bytes).fill(0xff);
  let retained = 0;
  for (const item of recordsOf(bytes)) {
    if (item.type !== 0) continue;
    for (let index = 0; index < item.data.byteLength; index += 1) {
      const address = item.address + index;
      if (address >= AVR_SPORE_REGION.start && address < AVR_SPORE_REGION.start + AVR_SPORE_REGION.bytes) {
        region[address - AVR_SPORE_REGION.start] = item.data[index];
        retained += 1;
      }
    }
  }
  if (retained !== AVR_SPORE_REGION.bytes
    || SPORE_MAGIC.some((byte, index) => region[index] !== byte)
    || region[SPORE_MAGIC.byteLength] !== SPORE_VERSION) {
    refuse("SporeMissing", "Pro Micro HEX omits its exact native Spore flash region");
  }
  const view = new DataView(region.buffer);
  const length = view.getUint16(SPORE_MAGIC.byteLength + 1, true);
  if (length < SPORE_FIXED_BYTES + SPORE_FIELD_NAMES.length * 2 || length > AVR_SPORE_REGION.bytes) {
    refuse("SporeBound", "Pro Micro Spore length is outside its reserved flash region");
  }
  let cursor = SPORE_FIXED_BYTES;
  const result = {
    protocol: 2,
    expires_at_millis: Number(view.getBigUint64(SPORE_MAGIC.byteLength + 3, true)),
    nonce: Array.from(region.subarray(SPORE_MAGIC.byteLength + 11, SPORE_MAGIC.byteLength + 43)),
    secret: Array.from(region.subarray(SPORE_MAGIC.byteLength + 43, SPORE_FIXED_BYTES)),
  };
  for (const name of SPORE_FIELD_NAMES) {
    const fieldLength = region[cursor];
    cursor += 1;
    if (fieldLength < 1 || fieldLength > MAX_ID_BYTES || cursor + fieldLength > length) {
      refuse("SporeMalformed", `Pro Micro Spore has malformed ${name}`);
    }
    result[name] = decoder.decode(region.subarray(cursor, cursor + fieldLength));
    cursor += fieldLength;
  }
  if (cursor !== length) refuse("SporeMalformed", "Pro Micro Spore has trailing provision bytes");
  return Object.freeze(result);
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
  const expectedArtifact = binding?.nativeSpore?.content_id;
  if (typeof expectedArtifact !== "string" || evidence.artifact_sha256 !== expectedArtifact) {
    refuse("StaleArtifact", "external programmer receipt names a different Body-bound HEX digest");
  }
  if (!Number.isSafeInteger(evidence.programmed_bytes) || evidence.programmed_bytes > APPLICATION_BYTES) {
    refuse("OversizedImage", "external programmer receipt exceeds admitted application flash");
  }
  if (!Number.isSafeInteger(evidence.maximum_address) || evidence.maximum_address > BOOT_REGION_START) {
    refuse("ProtectedBootRegion", "external programmer receipt overlaps the protected Caterina boot region");
  }
  if (evidence.programmed_bytes !== binding.nativeSpore.programmed_bytes
    || evidence.maximum_address !== binding.nativeSpore.maximum_address) {
    refuse("StaleArtifact", "external programmer receipt dimensions do not match the exact Body-bound HEX");
  }
  return Object.freeze({ ...evidence });
}

export function validateProMicroReleaseManifest(manifest, profile) {
  requireManifest(manifest, profile);
  return Object.freeze({ ...manifest });
}

function encodeProvision(prepared) {
  const fields = SPORE_FIELD_NAMES.map((name) => idBytes(prepared?.[name], name));
  const nonce = exactBytes(prepared?.invitation_nonce, 32, "invitation nonce");
  const secret = exactBytes(prepared?.invitation_secret, 32, "invitation secret");
  const expiry = prepared?.invitation_expires_at_millis;
  if (!Number.isSafeInteger(expiry) || expiry <= 0) {
    refuse("SporeMalformed", "invitation expiry is outside its exact integer bound");
  }
  const length = SPORE_FIXED_BYTES + fields.reduce((sum, field) => sum + 1 + field.byteLength, 0);
  if (length > AVR_SPORE_REGION.bytes) refuse("SporeBound", "AVR Body binding exceeds its reserved flash region");
  const bytes = new Uint8Array(AVR_SPORE_REGION.bytes).fill(0xff);
  bytes.set(SPORE_MAGIC);
  bytes[SPORE_MAGIC.byteLength] = SPORE_VERSION;
  const view = new DataView(bytes.buffer);
  view.setUint16(SPORE_MAGIC.byteLength + 1, length, true);
  view.setBigUint64(SPORE_MAGIC.byteLength + 3, BigInt(expiry), true);
  bytes.set(nonce, SPORE_MAGIC.byteLength + 11);
  bytes.set(secret, SPORE_MAGIC.byteLength + 43);
  let cursor = SPORE_FIXED_BYTES;
  for (const field of fields) {
    bytes[cursor] = field.byteLength;
    cursor += 1;
    bytes.set(field, cursor);
    cursor += field.byteLength;
  }
  return { bytes, length };
}

function recordsOf(value) {
  const source = decoder.decode(value instanceof Uint8Array ? value : new Uint8Array(value));
  let base = 0;
  const records = [];
  for (const raw of source.split(/\r?\n/)) {
    const line = raw.trim();
    if (!line) continue;
    const bytes = Uint8Array.from(line.slice(1).match(/../g) ?? [], (pair) => Number.parseInt(pair, 16));
    const count = bytes[0];
    const address = (bytes[1] << 8) | bytes[2];
    const type = bytes[3];
    const data = bytes.subarray(4, 4 + count);
    if (type === 2) base = ((data[0] << 8) | data[1]) << 4;
    if (type === 4) base = ((data[0] << 8) | data[1]) * 0x1_0000;
    records.push({ line, type, address: base + address, data });
  }
  return records;
}

function intelHexRecord(address, type, data = new Uint8Array()) {
  const bytes = [data.byteLength, address >> 8, address & 0xff, type, ...data];
  bytes.push((-bytes.reduce((sum, byte) => sum + byte, 0)) & 0xff);
  return `:${bytes.map((byte) => byte.toString(16).padStart(2, "0")).join("").toUpperCase()}`;
}

function exactBytes(value, length, name) {
  if (!Array.isArray(value) || value.length !== length
    || value.some((byte) => !Number.isInteger(byte) || byte < 0 || byte > 255)) {
    refuse("SporeMalformed", `${name} must retain exactly ${length} bytes`);
  }
  return Uint8Array.from(value);
}

function idBytes(value, name) {
  if (typeof value !== "string") refuse("SporeMalformed", `${name} is missing`);
  const bytes = encoder.encode(value);
  if (bytes.byteLength < 1 || bytes.byteLength > MAX_ID_BYTES
    || bytes.some((byte) => byte < 0x21 || byte > 0x7e)) {
    refuse("SporeBound", `${name} is not one bounded printable identity`);
  }
  return bytes;
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
    || manifest.flash?.boot_region_start !== BOOT_REGION_START || manifest.flash?.boot_region_bytes !== 4_096
    || manifest.flash?.spore_region_start !== AVR_SPORE_REGION.start
    || manifest.flash?.spore_region_bytes !== AVR_SPORE_REGION.bytes) {
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

async function sha256With(bytes, cryptoApi) {
  if (!cryptoApi?.subtle?.digest) refuse("DigestUnavailable", "SHA-256 is unavailable");
  const digest = new Uint8Array(await cryptoApi.subtle.digest("SHA-256", bytes));
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
