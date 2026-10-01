import { boundedRendezvousCandidates } from "./creche-rendezvous-candidates.mjs";

const encoder = new TextEncoder();
const decoder = new TextDecoder();
const MAGIC = encoder.encode("CONDUIT_SPORE_MEDIA@1\0");
const HEADER_BYTES = 32;
const TRAILER_BYTES = 4096;
const MAXIMUM_ARTIFACT_BYTES = 80 * 1024 * 1024;

export async function bindBodyProvisionedMedia({ prepared, imageBytes, filename, format, mediaType, provisionRegion = null }) {
  requirePrepared(prepared);
  const image = bytesOf(imageBytes);
  const maximumImageBytes = provisionRegion === null
    ? MAXIMUM_ARTIFACT_BYTES - TRAILER_BYTES
    : MAXIMUM_ARTIFACT_BYTES;
  if (image.byteLength < 512 || image.byteLength > maximumImageBytes) {
    throw new RangeError("native media IMAGE violates its admitted byte bound");
  }
  const provision = encoder.encode(JSON.stringify({
    schema: "conduit.spore/native-media-provision@1",
    image_bytes: image.byteLength,
    spore: prepared.spore_manifest,
    invitation_provision: {
      invitation_id: prepared.invitation_id,
      nonce: prepared.invitation_nonce,
      expires_at_millis: prepared.invitation_expires_at_millis,
      secret: prepared.invitation_secret,
      rendezvous_candidates: boundedRendezvousCandidates(prepared.rendezvous_candidates, {
        bodyId: prepared.body_id,
        invitationExpiresAtMillis: prepared.invitation_expires_at_millis,
      }),
    },
  }));
  if (provision.byteLength > TRAILER_BYTES - HEADER_BYTES) {
    throw new RangeError("native media provision exceeds its reserved trailer");
  }
  const embedded = provisionRegion !== null;
  const offset = embedded ? requireProvisionRegion(provisionRegion, image) : image.byteLength;
  const bytes = new Uint8Array(image.byteLength + (embedded ? 0 : TRAILER_BYTES));
  bytes.set(image);
  if (!embedded) bytes.fill(0xff, offset);
  bytes.set(MAGIC, offset);
  const view = new DataView(bytes.buffer, bytes.byteOffset + offset, TRAILER_BYTES);
  view.setUint32(24, 1, true);
  view.setUint32(28, provision.byteLength, true);
  bytes.fill(0xff, offset + HEADER_BYTES, offset + TRAILER_BYTES);
  bytes.set(provision, offset + HEADER_BYTES);
  const contentDigest = await sha256(bytes);
  return Object.freeze({
    schema: "conduit.spore/native-media@1",
    filename: requireFilename(filename),
    format: requireText(format, "format"),
    media_type: requireText(mediaType, "media type"),
    bytes,
    content_digest: contentDigest,
    image_content_digest: prepared.image_content_digest,
    image_bytes: image.byteLength,
    provision_offset: offset,
    provision_bytes: TRAILER_BYTES,
    provision_embedded: embedded,
  });
}

function requireProvisionRegion(region, image) {
  if (region?.schema !== "conduit.conduitos/spore-region@1"
    || region.encoding !== "conduit.spore/native-media-provision@1"
    || !Number.isSafeInteger(region.offset) || region.offset < 0
    || region.bytes !== TRAILER_BYTES || region.offset + region.bytes > image.byteLength) {
    throw new TypeError("native media provision region is missing or outside its exact IMAGE bound");
  }
  if (!MAGIC.every((byte, index) => image[region.offset + index] === byte)) {
    throw new TypeError("native media IMAGE lost its reviewed provision region marker");
  }
  const view = new DataView(image.buffer, image.byteOffset + region.offset, TRAILER_BYTES);
  if (view.getUint32(24, true) !== 0 || view.getUint32(28, true) !== 0
    || image.subarray(region.offset + HEADER_BYTES, region.offset + TRAILER_BYTES).some((byte) => byte !== 0xff)) {
    throw new TypeError("native media IMAGE provision region is not blank");
  }
  return region.offset;
}

export function readBodyProvisionedMedia(value) {
  const bytes = bytesOf(value);
  if (bytes.byteLength < 512 || bytes.byteLength > MAXIMUM_ARTIFACT_BYTES) {
    throw new RangeError("native media artifact violates its admitted byte bound");
  }
  const trailingOffset = bytes.byteLength - TRAILER_BYTES;
  const offset = trailingOffset >= 0 && hasMagic(bytes, trailingOffset)
    ? trailingOffset
    : findUniqueProvision(bytes);
  if (offset < 0) {
    throw new TypeError("native media artifact omitted its body provision trailer");
  }
  const view = new DataView(bytes.buffer, bytes.byteOffset + offset, TRAILER_BYTES);
  const version = view.getUint32(24, true);
  const length = view.getUint32(28, true);
  if (version !== 1 || length < 1 || length > TRAILER_BYTES - HEADER_BYTES) {
    throw new TypeError("native media provision header is malformed");
  }
  let provision;
  try {
    provision = JSON.parse(decoder.decode(bytes.subarray(offset + HEADER_BYTES, offset + HEADER_BYTES + length)));
  } catch (error) {
    throw new TypeError("native media provision is not valid JSON", { cause: error });
  }
  const embedded = offset !== trailingOffset;
  requireProvision(provision, embedded ? bytes.byteLength : offset);
  const image = embedded ? new Uint8Array(bytes) : bytes.slice(0, offset);
  if (embedded) {
    image.fill(0xff, offset, offset + TRAILER_BYTES);
    image.set(MAGIC, offset);
    new DataView(image.buffer, image.byteOffset + offset, TRAILER_BYTES).setUint32(24, 0, true);
    new DataView(image.buffer, image.byteOffset + offset, TRAILER_BYTES).setUint32(28, 0, true);
  }
  return Object.freeze({
    provision: Object.freeze(provision),
    image,
    provision_offset: offset,
    provision_bytes: TRAILER_BYTES,
    provision_embedded: embedded,
  });
}

function hasMagic(bytes, offset) {
  return offset >= 0 && MAGIC.every((byte, index) => bytes[offset + index] === byte);
}

function findUniqueProvision(bytes) {
  let found = -1;
  for (let offset = 0; offset + TRAILER_BYTES <= bytes.byteLength; offset += 1) {
    if (!hasMagic(bytes, offset)) continue;
    const view = new DataView(bytes.buffer, bytes.byteOffset + offset, TRAILER_BYTES);
    const version = view.getUint32(24, true);
    const length = view.getUint32(28, true);
    if (version !== 1 || length < 1 || length > TRAILER_BYTES - HEADER_BYTES
      || bytes.subarray(offset + HEADER_BYTES + length, offset + TRAILER_BYTES).some((byte) => byte !== 0xff)) continue;
    if (found >= 0) throw new TypeError("native media artifact contains ambiguous Body provision regions");
    found = offset;
  }
  return found;
}

function requirePrepared(prepared) {
  if (prepared?.spore_manifest?.schema !== "conduit.body/spore-manifest@2"
    || prepared.spore_manifest.spore_id !== prepared.spore_id
    || prepared.spore_manifest.image_content_digest !== prepared.image_content_digest) {
    throw new TypeError("native media requires one exact prepared spore manifest");
  }
}

function requireProvision(provision, imageBytes) {
  if (provision?.schema !== "conduit.spore/native-media-provision@1"
    || provision.image_bytes !== imageBytes
    || provision.spore?.schema !== "conduit.body/spore-manifest@2"
    || provision.spore.binding?.mode !== "self-joining"
    || provision.spore.binding.invitation_id !== provision.invitation_provision?.invitation_id
    || !Array.isArray(provision.invitation_provision?.nonce)
    || provision.invitation_provision.nonce.length !== 32
    || !Array.isArray(provision.invitation_provision?.secret)
    || provision.invitation_provision.secret.length !== 32
    || !Number.isSafeInteger(provision.invitation_provision?.expires_at_millis)) {
    throw new TypeError("native media provision lost its exact identities or bounds");
  }
  boundedRendezvousCandidates(provision.invitation_provision.rendezvous_candidates, {
    bodyId: provision.spore.body_id,
    invitationExpiresAtMillis: provision.invitation_provision.expires_at_millis,
  });
}

function requireFilename(value) {
  if (typeof value !== "string" || value.length < 5 || value.length > 192 || value.includes("/")) {
    throw new TypeError("native media requires one bounded filename");
  }
  return value;
}

function requireText(value, label) {
  if (typeof value !== "string" || value.length < 1 || value.length > 128) {
    throw new TypeError(`native media requires one bounded ${label}`);
  }
  return value;
}

function bytesOf(value) {
  if (value instanceof Uint8Array) return new Uint8Array(value);
  if (ArrayBuffer.isView(value)) return new Uint8Array(value.buffer.slice(value.byteOffset, value.byteOffset + value.byteLength));
  if (value instanceof ArrayBuffer) return new Uint8Array(value.slice(0));
  throw new TypeError("native media artifact is not bytes");
}

async function sha256(bytes) {
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", bytes));
  return `sha256:${Array.from(digest, (byte) => byte.toString(16).padStart(2, "0")).join("")}`;
}

export const NATIVE_MEDIA_PROVISION_BYTES = TRAILER_BYTES;
