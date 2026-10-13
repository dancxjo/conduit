const BLOCK_BYTES = 512;
const PAYLOAD_BYTES = 256;
const FLASH_PAGE_BYTES = 256;
const FLASH_SECTOR_BYTES = 4096;
const MAXIMUM_FLASH_BYTES = 2 * 1024 * 1024;
const MAXIMUM_UF2_BYTES = MAXIMUM_FLASH_BYTES * 2;
const FLASH_START = 0x10000000;
const FLASH_END = FLASH_START + MAXIMUM_FLASH_BYTES;
const SPORE_SECTOR_ADDRESS = FLASH_END - FLASH_SECTOR_BYTES;
const MAGIC_START_0 = 0x0a324655;
const MAGIC_START_1 = 0x9e5d5157;
const MAGIC_END = 0x0ab16f30;
const FAMILY_FLAG = 0x00002000;
const RP2040_FAMILY = 0xe48bff56;

export class Rp2040ImageRefusal extends Error {
  constructor(code, message) {
    super(message);
    this.name = "Rp2040ImageRefusal";
    this.code = code;
  }
}

function refuse(code, message) {
  throw new Rp2040ImageRefusal(code, message);
}

function bytesOf(value) {
  if (value instanceof Uint8Array) return value;
  if (ArrayBuffer.isView(value)) {
    return new Uint8Array(value.buffer, value.byteOffset, value.byteLength);
  }
  if (value instanceof ArrayBuffer) return new Uint8Array(value);
  refuse("MalformedImage", "RP2040 IMAGE content must be bytes");
}

function word(view, offset) {
  return view.getUint32(offset, true);
}

function alignUp(value, alignment) {
  return Math.ceil(value / alignment) * alignment;
}

export function parseRp2040Uf2(value, maximumTransferBytes) {
  const bytes = bytesOf(value);
  if (
    bytes.byteLength === 0
    || bytes.byteLength > MAXIMUM_UF2_BYTES
    || bytes.byteLength % BLOCK_BYTES !== 0
  ) {
    refuse("ImageSize", "RP2040 UF2 size is outside its finite block bound");
  }
  if (
    !Number.isInteger(maximumTransferBytes)
    || maximumTransferBytes < PAYLOAD_BYTES
    || maximumTransferBytes > 4096
    || maximumTransferBytes % FLASH_PAGE_BYTES !== 0
  ) {
    refuse("TransferBound", "RP2040 write bound must contain whole flash pages");
  }

  const blockCount = bytes.byteLength / BLOCK_BYTES;
  const pages = [];
  let previousEnd = null;
  for (let index = 0; index < blockCount; index += 1) {
    const offset = index * BLOCK_BYTES;
    const view = new DataView(bytes.buffer, bytes.byteOffset + offset, BLOCK_BYTES);
    if (
      word(view, 0) !== MAGIC_START_0
      || word(view, 4) !== MAGIC_START_1
      || word(view, 508) !== MAGIC_END
    ) {
      refuse("Uf2Magic", `RP2040 UF2 block ${index} has invalid magic`);
    }
    const flags = word(view, 8);
    if (flags !== FAMILY_FLAG || word(view, 28) !== RP2040_FAMILY) {
      refuse("ImageCompatibility", `UF2 block ${index} is not an exact RP2040 family block`);
    }
    const address = word(view, 12);
    const payloadBytes = word(view, 16);
    if (payloadBytes !== PAYLOAD_BYTES || address % FLASH_PAGE_BYTES !== 0) {
      refuse("FlashAlignment", `UF2 block ${index} is not one exact flash page`);
    }
    if (
      address < FLASH_START
      || address + payloadBytes > FLASH_END
      || word(view, 20) !== index
      || word(view, 24) !== blockCount
    ) {
      refuse("Uf2Sequence", `UF2 block ${index} has stale address or sequence truth`);
    }
    if (previousEnd !== null && address !== previousEnd && address !== SPORE_SECTOR_ADDRESS) {
      refuse("SparseImage", "RP2040 UF2 admits only one exact native Spore bootstrap sector gap");
    }
    if (previousEnd !== null && address < previousEnd) {
      refuse("Uf2Sequence", `RP2040 UF2 block ${index} overlaps an earlier flash page`);
    }
    previousEnd = address + payloadBytes;
    pages.push({
      address,
      bytes: bytes.slice(offset + 32, offset + 32 + payloadBytes),
    });
  }

  const chunks = [];
  for (let index = 0; index < pages.length;) {
    const first = pages[index];
    let take = 1;
    const maximumPages = maximumTransferBytes / PAYLOAD_BYTES;
    while (take < maximumPages && index + take < pages.length
      && pages[index + take].address === first.address + take * PAYLOAD_BYTES) {
      take += 1;
    }
    const chunkBytes = new Uint8Array(take * PAYLOAD_BYTES);
    for (let page = 0; page < take; page += 1) {
      chunkBytes.set(pages[index + page].bytes, page * PAYLOAD_BYTES);
    }
    chunks.push(Object.freeze({ address: first.address, bytes: chunkBytes }));
    index += take;
  }

  const startAddress = pages[0].address;
  const endAddress = previousEnd;
  const eraseStart = Math.floor(startAddress / FLASH_SECTOR_BYTES) * FLASH_SECTOR_BYTES;
  const eraseEnd = alignUp(endAddress, FLASH_SECTOR_BYTES);
  if (eraseStart !== startAddress) {
    refuse("FlashAlignment", "RP2040 IMAGE must begin on a flash erase boundary");
  }
  return Object.freeze({
    uf2Bytes: bytes.byteLength,
    imageBytes: pages.length * PAYLOAD_BYTES,
    blockCount,
    startAddress,
    endAddress,
    eraseStart,
    eraseBytes: eraseEnd - eraseStart,
    chunks: Object.freeze(chunks),
  });
}

export async function sha256ContentId(value, cryptoApi = globalThis.crypto) {
  const bytes = bytesOf(value);
  if (!cryptoApi?.subtle?.digest) refuse("DigestUnavailable", "SHA-256 is unavailable");
  const digest = new Uint8Array(await cryptoApi.subtle.digest("SHA-256", bytes));
  return `sha256:${Array.from(digest, (byte) => byte.toString(16).padStart(2, "0")).join("")}`;
}

export const RP2040_UF2 = Object.freeze({
  blockBytes: BLOCK_BYTES,
  payloadBytes: PAYLOAD_BYTES,
  familyId: RP2040_FAMILY,
  familyFlag: FAMILY_FLAG,
  flashStart: FLASH_START,
  flashEnd: FLASH_END,
  sporeSectorAddress: SPORE_SECTOR_ADDRESS,
  sporeSectorBytes: FLASH_SECTOR_BYTES,
});
