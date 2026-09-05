const encoder = new TextEncoder();
const decoder = new TextDecoder();
const LOCAL_FILE = 0x04034b50;
const CENTRAL_FILE = 0x02014b50;
const END = 0x06054b50;
const UTF8 = 0x0800;
const MAXIMUM_FILES = 17;
const MAXIMUM_BYTES = 48 * 1024 * 1024;
const PROVISION_PATH = "conduit-spore.json";
const CRC_TABLE = createCrcTable();

export async function createBodyBoundZip({ prepared, release, filename }) {
  requirePrepared(prepared);
  if (!Array.isArray(release?.payloads) || release.payloads.length < 1
    || release.payloads.length >= MAXIMUM_FILES) {
    throw new TypeError("native ZIP requires one finite reviewed release payload set");
  }
  const provision = encoder.encode(JSON.stringify({
    schema: "conduit.spore/native-package-provision@1",
    spore: prepared.spore_manifest,
    invitation_provision: {
      invitation_id: prepared.invitation_id,
      nonce: prepared.invitation_nonce,
      expires_at_millis: prepared.invitation_expires_at_millis,
      secret: prepared.invitation_secret,
    },
  }));
  const entries = release.payloads.map(({ path, bytes, media_type: mediaType }) => ({
    path,
    bytes: bytesOf(bytes),
    mode: ["application/vnd.conduit.host+executable", "application/vnd.microsoft.portable-executable"].includes(mediaType)
      ? 0o100755
      : 0o100644,
  }));
  entries.push({ path: PROVISION_PATH, bytes: provision, mode: 0o100644 });
  const bytes = writeZip(entries);
  if (bytes.byteLength > MAXIMUM_BYTES) throw new RangeError("native ZIP exceeds its admitted byte bound");
  const contentDigest = await sha256(bytes);
  return Object.freeze({
    schema: "conduit.spore/native-zip@1",
    filename: requireFilename(filename),
    format: "zip",
    media_type: "application/zip",
    bytes,
    content_digest: contentDigest,
    image_content_digest: prepared.image_content_digest,
    files: Object.freeze(entries.map(({ path, bytes: entry, mode }) => Object.freeze({ path, bytes: entry.byteLength, mode }))),
  });
}

export function readBodyBoundZip(value) {
  const bytes = bytesOf(value);
  if (bytes.byteLength < 22 || bytes.byteLength > MAXIMUM_BYTES) {
    throw new RangeError("native ZIP violates its admitted byte bound");
  }
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const endOffset = bytes.byteLength - 22;
  if (view.getUint32(endOffset, true) !== END || view.getUint16(endOffset + 20, true) !== 0) {
    throw new TypeError("native ZIP has no exact terminal directory");
  }
  const count = view.getUint16(endOffset + 10, true);
  const centralBytes = view.getUint32(endOffset + 12, true);
  const centralOffset = view.getUint32(endOffset + 16, true);
  if (count < 2 || count > MAXIMUM_FILES || centralOffset + centralBytes !== endOffset) {
    throw new TypeError("native ZIP central directory is malformed or unbounded");
  }
  const entries = new Map();
  const modes = new Map();
  let cursor = centralOffset;
  for (let index = 0; index < count; index += 1) {
    if (cursor + 46 > endOffset || view.getUint32(cursor, true) !== CENTRAL_FILE
      || view.getUint16(cursor + 10, true) !== 0) {
      throw new TypeError("native ZIP central entry is malformed");
    }
    const expectedCrc = view.getUint32(cursor + 16, true);
    const compressed = view.getUint32(cursor + 20, true);
    const size = view.getUint32(cursor + 24, true);
    const nameLength = view.getUint16(cursor + 28, true);
    const extraLength = view.getUint16(cursor + 30, true);
    const commentLength = view.getUint16(cursor + 32, true);
    const localOffset = view.getUint32(cursor + 42, true);
    const mode = view.getUint32(cursor + 38, true) >>> 16;
    const name = decodeName(bytes, cursor + 46, nameLength);
    cursor += 46 + nameLength + extraLength + commentLength;
    if (compressed !== size || entries.has(name) || localOffset + 30 > centralOffset
      || view.getUint32(localOffset, true) !== LOCAL_FILE || view.getUint16(localOffset + 8, true) !== 0) {
      throw new TypeError("native ZIP entry layout is inconsistent");
    }
    const localNameLength = view.getUint16(localOffset + 26, true);
    const localExtraLength = view.getUint16(localOffset + 28, true);
    const localName = decodeName(bytes, localOffset + 30, localNameLength);
    const dataOffset = localOffset + 30 + localNameLength + localExtraLength;
    if (localName !== name || dataOffset + size > centralOffset) {
      throw new TypeError("native ZIP local entry is inconsistent");
    }
    const entry = bytes.slice(dataOffset, dataOffset + size);
    if (crc32(entry) !== expectedCrc) throw new TypeError(`native ZIP entry ${name} failed CRC-32`);
    entries.set(name, entry);
    modes.set(name, mode);
  }
  if (cursor !== endOffset || !entries.has(PROVISION_PATH)) {
    throw new TypeError("native ZIP omitted its exact Body provision");
  }
  let provision;
  try {
    provision = JSON.parse(decoder.decode(entries.get(PROVISION_PATH)));
  } catch (error) {
    throw new TypeError("native ZIP Body provision is not valid JSON", { cause: error });
  }
  requireProvisionIdentities(provision);
  return Object.freeze({ entries, modes, provision: Object.freeze(provision) });
}

function writeZip(entries) {
  const names = new Set();
  const locals = [];
  const centrals = [];
  let localOffset = 0;
  for (const { path, bytes, mode } of entries) {
    const name = requirePath(path, names);
    const nameBytes = encoder.encode(name);
    const crc = crc32(bytes);
    const local = new Uint8Array(30 + nameBytes.byteLength + bytes.byteLength);
    const localView = new DataView(local.buffer);
    localView.setUint32(0, LOCAL_FILE, true);
    localView.setUint16(4, 20, true);
    localView.setUint16(6, UTF8, true);
    localView.setUint32(14, crc, true);
    localView.setUint16(12, 0x0021, true);
    localView.setUint32(18, bytes.byteLength, true);
    localView.setUint32(22, bytes.byteLength, true);
    localView.setUint16(26, nameBytes.byteLength, true);
    local.set(nameBytes, 30);
    local.set(bytes, 30 + nameBytes.byteLength);
    locals.push(local);

    const central = new Uint8Array(46 + nameBytes.byteLength);
    const centralView = new DataView(central.buffer);
    centralView.setUint32(0, CENTRAL_FILE, true);
    centralView.setUint16(4, 0x0314, true);
    centralView.setUint16(6, 20, true);
    centralView.setUint16(8, UTF8, true);
    centralView.setUint32(16, crc, true);
    centralView.setUint16(14, 0x0021, true);
    centralView.setUint32(20, bytes.byteLength, true);
    centralView.setUint32(24, bytes.byteLength, true);
    centralView.setUint16(28, nameBytes.byteLength, true);
    centralView.setUint32(42, localOffset, true);
    centralView.setUint32(38, (mode << 16) >>> 0, true);
    central.set(nameBytes, 46);
    centrals.push(central);
    localOffset += local.byteLength;
  }
  const centralBytes = centrals.reduce((sum, part) => sum + part.byteLength, 0);
  const end = new Uint8Array(22);
  const endView = new DataView(end.buffer);
  endView.setUint32(0, END, true);
  endView.setUint16(8, entries.length, true);
  endView.setUint16(10, entries.length, true);
  endView.setUint32(12, centralBytes, true);
  endView.setUint32(16, localOffset, true);
  return joinBytes([...locals, ...centrals, end], localOffset + centralBytes + end.byteLength);
}

function requirePrepared(prepared) {
  if (prepared?.spore_manifest?.schema !== "conduit.body/spore-manifest@2"
    || prepared.spore_manifest.spore_id !== prepared.spore_id
    || prepared.spore_manifest.image_content_digest !== prepared.image_content_digest) {
    throw new TypeError("native ZIP requires one exact prepared spore manifest");
  }
}

function requireProvisionIdentities(provision) {
  if (provision?.schema !== "conduit.spore/native-package-provision@1"
    || provision.spore?.schema !== "conduit.body/spore-manifest@2"
    || provision.spore.binding?.mode !== "self-joining"
    || provision.spore.binding.invitation_id !== provision.invitation_provision?.invitation_id
    || !Array.isArray(provision.invitation_provision?.nonce)
    || provision.invitation_provision.nonce.length !== 32
    || !Array.isArray(provision.invitation_provision?.secret)
    || provision.invitation_provision.secret.length !== 32
    || !Number.isSafeInteger(provision.invitation_provision?.expires_at_millis)) {
    throw new TypeError("native ZIP Body provision lost its exact identities");
  }
}

function requireFilename(value) {
  if (typeof value !== "string" || value.length < 5 || value.length > 192
    || value.includes("/") || !value.endsWith(".zip")) {
    throw new TypeError("native ZIP requires one bounded .zip filename");
  }
  return value;
}

function requirePath(value, names) {
  if (typeof value !== "string" || value.length < 1 || value.length > 256
    || value.includes("/") || value.includes("\\") || value === "." || value === ".." || names.has(value)) {
    throw new TypeError("native ZIP entry path is malformed, nested, or duplicated");
  }
  names.add(value);
  return value;
}

function decodeName(bytes, offset, length) {
  if (length < 1 || length > 256 || offset + length > bytes.byteLength) {
    throw new TypeError("native ZIP entry name violates its bound");
  }
  return decoder.decode(bytes.subarray(offset, offset + length));
}

function bytesOf(value) {
  if (value instanceof Uint8Array) return new Uint8Array(value);
  if (ArrayBuffer.isView(value)) return new Uint8Array(value.buffer.slice(value.byteOffset, value.byteOffset + value.byteLength));
  if (value instanceof ArrayBuffer) return new Uint8Array(value.slice(0));
  throw new TypeError("native ZIP entry is not bytes");
}

function joinBytes(parts, length) {
  const result = new Uint8Array(length);
  let cursor = 0;
  for (const part of parts) { result.set(part, cursor); cursor += part.byteLength; }
  return result;
}

function createCrcTable() {
  return Object.freeze(Array.from({ length: 256 }, (_, value) => {
    let crc = value;
    for (let bit = 0; bit < 8; bit += 1) crc = (crc >>> 1) ^ ((crc & 1) ? 0xedb88320 : 0);
    return crc >>> 0;
  }));
}

function crc32(bytes) {
  let crc = 0xffffffff;
  for (const byte of bytes) crc = (crc >>> 8) ^ CRC_TABLE[(crc ^ byte) & 0xff];
  return (crc ^ 0xffffffff) >>> 0;
}

async function sha256(bytes) {
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", bytes));
  return `sha256:${Array.from(digest, (byte) => byte.toString(16).padStart(2, "0")).join("")}`;
}
