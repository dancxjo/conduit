import { md5Hex } from "./md5.mjs";

const MAXIMUM_IMAGE_BYTES = 4 * 1024 * 1024;
const MAXIMUM_SEGMENTS = 8;
const MAXIMUM_FLASH_ADDRESS = 16 * 1024 * 1024;
const MERGED_SPORE_BYTES = 4 * 1024 * 1024;
export const ESP32_SPORE_REGION = Object.freeze({ start: MERGED_SPORE_BYTES - 4096, bytes: 4096 });
const encoder = new TextEncoder();
const decoder = new TextDecoder("utf-8", { fatal: true });
const SPORE_MAGIC = encoder.encode("CONDUIT_SPORE@1\0");
const SPORE_VERSION = 1;
const SPORE_FIXED_BYTES = SPORE_MAGIC.byteLength + 1 + 2 + 8 + 32 + 32;
const MAX_ID_BYTES = 128;
const SPORE_FIELD_NAMES = Object.freeze(["spore_id", "image_id", "invitation_id", "body_id"]);
const TARGET_IMAGE_HEADERS = Object.freeze({
  "esp32/xtensa-lx6/hw-463-esp-wroom-32": Object.freeze({ chipId: 0, bootloaderOffset: 0x1000 }),
  "esp32/riscv32imc/usb-dcf8355d-esp32-c3": Object.freeze({ chipId: 5, bootloaderOffset: 0 }),
  "esp32/xtensa-lx7/usb-54e2006398-esp32-s3": Object.freeze({ chipId: 9, bootloaderOffset: 0 }),
});

export class Esp32ImageRefusal extends Error {
  constructor(code, message) {
    super(message);
    this.name = "Esp32ImageRefusal";
    this.code = code;
  }
}

function refuse(code, message) {
  throw new Esp32ImageRefusal(code, message);
}

function bytesOf(value) {
  if (value instanceof Uint8Array) return new Uint8Array(value);
  if (ArrayBuffer.isView(value)) return new Uint8Array(value.buffer.slice(value.byteOffset, value.byteOffset + value.byteLength));
  if (value instanceof ArrayBuffer) return new Uint8Array(value.slice(0));
  refuse("ImageBytes", "ESP32 IMAGE segments must contain bytes");
}

function u32(value) {
  const bytes = new Uint8Array(4);
  new DataView(bytes.buffer).setUint32(0, value >>> 0, true);
  return bytes;
}

export async function sha256ContentId(targetId, segments, cryptoApi = globalThis.crypto) {
  const target = new TextEncoder().encode(targetId);
  const length = 4 + target.length + segments.reduce((sum, segment) => sum + 8 + segment.bytes.length, 0);
  const framed = new Uint8Array(length);
  let cursor = 0;
  framed.set(u32(target.length), cursor); cursor += 4;
  framed.set(target, cursor); cursor += target.length;
  for (const segment of segments) {
    framed.set(u32(segment.offset), cursor); cursor += 4;
    framed.set(u32(segment.bytes.length), cursor); cursor += 4;
    framed.set(segment.bytes, cursor); cursor += segment.bytes.length;
  }
  const digest = new Uint8Array(await cryptoApi.subtle.digest("SHA-256", framed));
  return `sha256:${Array.from(digest, (byte) => byte.toString(16).padStart(2, "0")).join("")}`;
}

export async function sha256Bytes(value, cryptoApi = globalThis.crypto) {
  const bytes = bytesOf(value);
  if (!cryptoApi?.subtle?.digest) refuse("DigestUnavailable", "SHA-256 is unavailable");
  const digest = new Uint8Array(await cryptoApi.subtle.digest("SHA-256", bytes));
  return `sha256:${Array.from(digest, (byte) => byte.toString(16).padStart(2, "0")).join("")}`;
}

export async function parseEsp32Image({ targetId, segments, maximumTransferBytes, cryptoApi }) {
  const expectedHeader = TARGET_IMAGE_HEADERS[targetId];
  if (!expectedHeader) refuse("WrongTarget", "ESP32 IMAGE target is unsupported");
  if (!Array.isArray(segments) || segments.length < 1 || segments.length > MAXIMUM_SEGMENTS) {
    refuse("SegmentCount", "ESP32 IMAGE must contain one finite non-empty segment set");
  }
  if (!Number.isSafeInteger(maximumTransferBytes) || maximumTransferBytes < 2098) {
    refuse("TransferBound", "serial Base cannot carry a worst-case escaped 1024-byte flash block");
  }
  const parsed = segments.map((segment) => {
    const offset = segment?.offset;
    const bytes = bytesOf(segment?.bytes);
    if (!Number.isSafeInteger(offset) || offset < 0 || offset % 4096 !== 0) {
      refuse("FlashOffset", "ESP32 IMAGE segment offset must be a non-negative 4 KiB boundary");
    }
    if (bytes.length < 1 || offset + bytes.length > MAXIMUM_FLASH_ADDRESS) {
      refuse("SegmentBounds", "ESP32 IMAGE segment exceeds the admitted flash address bound");
    }
    return Object.freeze({ offset, bytes, md5: md5Hex(bytes) });
  }).sort((left, right) => left.offset - right.offset);
  let totalBytes = 0;
  for (let index = 0; index < parsed.length; index += 1) {
    const segment = parsed[index];
    totalBytes += segment.bytes.length;
    if (totalBytes > MAXIMUM_IMAGE_BYTES) refuse("ImageTooLarge", "ESP32 IMAGE exceeds its 4 MiB payload bound");
    if (index > 0 && parsed[index - 1].offset + parsed[index - 1].bytes.length > segment.offset) {
      refuse("SegmentOverlap", "ESP32 IMAGE segments overlap");
    }
  }
  const bootloader = parsed.find((segment) => segment.offset <= expectedHeader.bootloaderOffset
    && segment.offset + segment.bytes.length >= expectedHeader.bootloaderOffset + 24);
  const headerOffset = bootloader ? expectedHeader.bootloaderOffset - bootloader.offset : -1;
  if (!bootloader || headerOffset < 0 || bootloader.bytes[headerOffset] !== 0xe9) {
    refuse("ImageHeader", "ESP32 IMAGE lacks the target's bootloader image header at its exact flash offset");
  }
  const header = new DataView(bootloader.bytes.buffer, bootloader.bytes.byteOffset + headerOffset, 24);
  const chipId = header.getUint16(12, true);
  if (chipId !== expectedHeader.chipId) {
    refuse("IncompatibleImage", `ESP32 IMAGE chip id ${chipId} is incompatible with its selected target`);
  }
  const contentId = await sha256ContentId(targetId, parsed, cryptoApi);
  return Object.freeze({ targetId, segments: Object.freeze(parsed), totalBytes, contentId });
}

export async function bindEsp32BodySpore({ targetId, segments, prepared, cryptoApi = globalThis.crypto }) {
  if (prepared?.output !== "esp32-image" || prepared?.target_id !== targetId) {
    refuse("SporeTarget", "prepared Body binding is not for the exact ESP32 target");
  }
  const generic = await parseEsp32Image({ targetId, segments, maximumTransferBytes: 4096, cryptoApi });
  if (generic.segments.some((segment) => segment.offset + segment.bytes.byteLength > ESP32_SPORE_REGION.start)) {
    refuse("SporeOverlap", "generic ESP32 IMAGE overlaps the reserved native Spore sector");
  }
  const bytes = new Uint8Array(MERGED_SPORE_BYTES).fill(0xff);
  for (const segment of generic.segments) bytes.set(segment.bytes, segment.offset);
  const provision = encodeProvision(prepared);
  bytes.set(provision.bytes, ESP32_SPORE_REGION.start);
  const native = await parseEsp32Image({
    targetId,
    segments: [{ offset: 0, bytes }],
    maximumTransferBytes: 4096,
    cryptoApi,
  });
  const recovered = readEsp32BodySpore(bytes);
  for (const name of SPORE_FIELD_NAMES) {
    if (recovered[name] !== prepared[name]) refuse("SporeBinding", `native ESP32 Spore lost ${name}`);
  }
  return Object.freeze({
    schema: "conduit.esp32/native-body-spore@1",
    format: "espressif-merged-image",
    bytes,
    segments: native.segments,
    content_id: await sha256Bytes(bytes, cryptoApi),
    deployment_content_id: native.contentId,
    image_content_id: prepared.image_content_digest,
    spore_id: prepared.spore_id,
    bootstrap_bytes: provision.length,
    bootstrap_flash_address: ESP32_SPORE_REGION.start,
  });
}

export function readEsp32BodySpore(value) {
  const bytes = value instanceof Uint8Array ? value : new Uint8Array(value);
  if (bytes.byteLength !== MERGED_SPORE_BYTES) refuse("SporeBound", "ESP32 Spore is not one exact 4 MiB merged flash image");
  const region = bytes.subarray(ESP32_SPORE_REGION.start, ESP32_SPORE_REGION.start + ESP32_SPORE_REGION.bytes);
  if (SPORE_MAGIC.some((byte, index) => region[index] !== byte)
    || region[SPORE_MAGIC.byteLength] !== SPORE_VERSION) {
    refuse("SporeMissing", "ESP32 image omits its exact native Spore flash sector");
  }
  const view = new DataView(region.buffer, region.byteOffset, region.byteLength);
  const length = view.getUint16(SPORE_MAGIC.byteLength + 1, true);
  if (length < SPORE_FIXED_BYTES + SPORE_FIELD_NAMES.length * 2 || length > ESP32_SPORE_REGION.bytes) {
    refuse("SporeBound", "ESP32 Spore length is outside its reserved flash sector");
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
      refuse("SporeMalformed", `ESP32 Spore has malformed ${name}`);
    }
    result[name] = decoder.decode(region.subarray(cursor, cursor + fieldLength));
    cursor += fieldLength;
  }
  if (cursor !== length) refuse("SporeMalformed", "ESP32 Spore has trailing provision bytes");
  return Object.freeze(result);
}

function encodeProvision(prepared) {
  const fields = SPORE_FIELD_NAMES.map((name) => idBytes(prepared?.[name], name));
  const nonce = exactBytes(prepared?.invitation_nonce, 32, "invitation nonce");
  const secret = exactBytes(prepared?.invitation_secret, 32, "invitation secret");
  const expiry = prepared?.invitation_expires_at_millis;
  if (!Number.isSafeInteger(expiry) || expiry <= 0) refuse("SporeMalformed", "invitation expiry is outside its exact integer bound");
  const length = SPORE_FIXED_BYTES + fields.reduce((sum, field) => sum + 1 + field.byteLength, 0);
  if (length > ESP32_SPORE_REGION.bytes) refuse("SporeBound", "ESP32 Body binding exceeds its reserved flash sector");
  const bytes = new Uint8Array(ESP32_SPORE_REGION.bytes).fill(0xff);
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

export const ESP32_IMAGE_BOUNDS = Object.freeze({
  maximumImageBytes: MAXIMUM_IMAGE_BYTES,
  maximumSegments: MAXIMUM_SEGMENTS,
  maximumFlashAddress: MAXIMUM_FLASH_ADDRESS,
  flashBlockBytes: 1024,
});
