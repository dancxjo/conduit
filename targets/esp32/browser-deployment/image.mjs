import { md5Hex } from "./md5.mjs";

const MAXIMUM_IMAGE_BYTES = 4 * 1024 * 1024;
const MAXIMUM_SEGMENTS = 8;
const MAXIMUM_FLASH_ADDRESS = 16 * 1024 * 1024;
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
  const bootloader = parsed.find((segment) => segment.offset === expectedHeader.bootloaderOffset);
  if (!bootloader || bootloader.bytes.length < 24 || bootloader.bytes[0] !== 0xe9) {
    refuse("ImageHeader", "ESP32 IMAGE lacks the target's bootloader image header at its exact flash offset");
  }
  const header = new DataView(bootloader.bytes.buffer, bootloader.bytes.byteOffset, 24);
  const chipId = header.getUint16(12, true);
  if (chipId !== expectedHeader.chipId) {
    refuse("IncompatibleImage", `ESP32 IMAGE chip id ${chipId} is incompatible with its selected target`);
  }
  const contentId = await sha256ContentId(targetId, parsed, cryptoApi);
  return Object.freeze({ targetId, segments: Object.freeze(parsed), totalBytes, contentId });
}

export const ESP32_IMAGE_BOUNDS = Object.freeze({
  maximumImageBytes: MAXIMUM_IMAGE_BYTES,
  maximumSegments: MAXIMUM_SEGMENTS,
  maximumFlashAddress: MAXIMUM_FLASH_ADDRESS,
  flashBlockBytes: 1024,
});
