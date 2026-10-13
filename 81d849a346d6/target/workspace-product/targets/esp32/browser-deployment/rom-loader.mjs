import { createSlipReader, encodeSlip, littleEndianWords, Esp32SlipRefusal } from "./slip.mjs";

const SYNC = 0x08;
const READ_REG = 0x0a;
const SPI_SET_PARAMS = 0x0b;
const SPI_ATTACH = 0x0d;
const FLASH_BEGIN = 0x02;
const FLASH_DATA = 0x03;
const FLASH_END = 0x04;
const FLASH_MD5 = 0x13;
const BLOCK_BYTES = 1024;
const CHECKSUM_MAGIC = 0xef;

const TARGETS = Object.freeze({
  "esp32/xtensa-lx6/hw-463-esp-wroom-32": Object.freeze({
    chip: "esp32",
    magicRegister: 0x40001000,
    magicValues: Object.freeze([0x00f01d83]),
    flashBytes: 4 * 1024 * 1024,
    extendedBegin: false,
  }),
  "esp32/riscv32imc/usb-dcf8355d-esp32-c3": Object.freeze({
    chip: "esp32c3",
    magicRegister: 0x40001000,
    magicValues: Object.freeze([0x6921506f, 0x1b31506f, 0x4881606f, 0x4361606f]),
    flashBytes: 4 * 1024 * 1024,
    extendedBegin: true,
  }),
  "esp32/xtensa-lx7/usb-54e2006398-esp32-s3": Object.freeze({
    chip: "esp32s3",
    magicRegister: 0x40001000,
    magicValues: Object.freeze([0x00000009]),
    flashBytes: 16 * 1024 * 1024,
    extendedBegin: true,
  }),
});

export class Esp32RomRefusal extends Error {
  constructor(code, message, cause = undefined) {
    super(message, cause ? { cause } : undefined);
    this.name = "Esp32RomRefusal";
    this.code = code;
  }
}

function refuse(code, message, cause) {
  throw new Esp32RomRefusal(code, message, cause);
}

function checksum(bytes) {
  return bytes.reduce((value, byte) => value ^ byte, CHECKSUM_MAGIC) >>> 0;
}

function commandPacket(operation, data = new Uint8Array(), commandChecksum = 0) {
  const packet = new Uint8Array(8 + data.length);
  const view = new DataView(packet.buffer);
  view.setUint8(0, 0);
  view.setUint8(1, operation);
  view.setUint16(2, data.length, true);
  view.setUint32(4, commandChecksum, true);
  packet.set(data, 8);
  return packet;
}

function responsePacket(frame, operation) {
  if (frame.length < 12) refuse("ResponseLength", "ROM-loader response is shorter than its header and status");
  const view = new DataView(frame.buffer, frame.byteOffset, frame.byteLength);
  const size = view.getUint16(2, true);
  if (view.getUint8(0) !== 1 || view.getUint8(1) !== operation || size !== frame.length - 8) {
    refuse("ResponseIdentity", "ROM-loader response direction, command, or length is stale");
  }
  const data = frame.subarray(8);
  const status = data.subarray(data.length - 4);
  if (status[0] !== 0) refuse("RomStatus", `ROM-loader command 0x${operation.toString(16)} refused ${status[0]}`);
  return Object.freeze({ value: view.getUint32(4, true), data: data.subarray(0, data.length - 4) });
}

function paddedBlock(bytes, offset) {
  const block = new Uint8Array(BLOCK_BYTES);
  block.fill(0xff);
  block.set(bytes.subarray(offset, offset + BLOCK_BYTES));
  return block;
}

export function requiredEsp32Transfers(image) {
  const blocks = image.segments.reduce((count, segment) => count + Math.ceil(segment.bytes.length / BLOCK_BYTES), 0);
  const commands = 1 + 1 + 1 + 1 + image.segments.length * 2 + blocks + 1;
  return Object.freeze({
    commands,
    maximumWrites: commands,
    maximumReads: (commands + 7) * 8,
    blocks,
  });
}

export async function deployEsp32Rom({ base, image, targetId, progress = () => {} }) {
  const target = TARGETS[targetId];
  if (!target) refuse("WrongTarget", "ESP32 target is not supported by this adapter");
  const maximumFrameBytes = base.evidence().transfer_bounds.maximum_transfer_bytes;
  const reader = createSlipReader({ base, maximumFrameBytes, maximumFragmentsPerFrame: 8 });
  let commands = 0;

  async function writeCommand(operation, data = new Uint8Array(), commandChecksum = 0) {
    const encoded = encodeSlip(commandPacket(operation, data, commandChecksum), maximumFrameBytes);
    await base.write(encoded);
    commands += 1;
    progress({ operation, commands });
  }

  async function readResponse(operation) {
    return responsePacket(await reader.readFrame(), operation);
  }

  async function checked(operation, data = new Uint8Array(), commandChecksum = 0) {
    await writeCommand(operation, data, commandChecksum);
    return readResponse(operation);
  }

  try {
    const sync = new Uint8Array(36);
    sync.set([0x07, 0x07, 0x12, 0x20]);
    sync.fill(0x55, 4);
    await writeCommand(SYNC, sync);
    const syncResponses = [];
    for (let index = 0; index < 8; index += 1) syncResponses.push(await readResponse(SYNC));
    if (syncResponses.some((response) => response.value === 0)) {
      refuse("StubLoader", "deployment requires the target ROM loader, not a retained flasher stub");
    }

    const observed = await checked(READ_REG, littleEndianWords(target.magicRegister));
    if (!target.magicValues.includes(observed.value >>> 0)) {
      refuse("WrongChip", `ROM-loader chip observation 0x${(observed.value >>> 0).toString(16)} is incompatible with ${target.chip}`);
    }

    await checked(SPI_ATTACH, littleEndianWords(0, 0));
    await checked(SPI_SET_PARAMS, littleEndianWords(0, target.flashBytes, 65536, 4096, 256, 0xffff));

    let completedBytes = 0;
    for (const segment of image.segments) {
      const blockCount = Math.ceil(segment.bytes.length / BLOCK_BYTES);
      const begin = [segment.bytes.length, blockCount, BLOCK_BYTES, segment.offset];
      if (target.extendedBegin) begin.push(0);
      await checked(FLASH_BEGIN, littleEndianWords(...begin));
      for (let sequence = 0; sequence < blockCount; sequence += 1) {
        const block = paddedBlock(segment.bytes, sequence * BLOCK_BYTES);
        const data = new Uint8Array(16 + block.length);
        data.set(littleEndianWords(block.length, sequence, 0, 0));
        data.set(block, 16);
        await checked(FLASH_DATA, data, checksum(block));
        completedBytes += Math.min(BLOCK_BYTES, segment.bytes.length - sequence * BLOCK_BYTES);
        progress({ operation: FLASH_DATA, commands, completedBytes });
      }
      const md5 = await checked(FLASH_MD5, littleEndianWords(segment.offset, segment.bytes.length, 0, 0));
      const observedMd5 = new TextDecoder().decode(md5.data);
      if (observedMd5 !== segment.md5) refuse("VerificationFailed", "ROM-loader flash MD5 differs from the sealed IMAGE segment");
    }
    await checked(FLASH_END, littleEndianWords(0));
    return Object.freeze({ commands, completedBytes, chip: target.chip, chipMagic: observed.value >>> 0 });
  } catch (error) {
    if (error instanceof Esp32RomRefusal) throw error;
    if (error instanceof Esp32SlipRefusal) refuse(error.code, "ESP32 ROM-loader framing failed", error);
    refuse("DeploymentFailed", "ESP32 ROM-loader deployment failed", error);
  }
}

export const ESP32_ROM_TARGETS = TARGETS;
