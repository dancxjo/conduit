import { parseRp2040Uf2, RP2040_UF2, sha256ContentId } from "./uf2.mjs";

const MAXIMUM_ARTIFACT_BYTES = 2 * 1024 * 1024;
const MAXIMUM_CONFIGURATION_BYTES = 192;
const UF2_BLOCK_BYTES = 512;
const UF2_PAYLOAD_BYTES = 256;
const encoder = new TextEncoder();
const SPORE_MAGIC = encoder.encode("CONDUIT_SPORE@1\0");
const SPORE_HEADER_BYTES = SPORE_MAGIC.byteLength + 4;

export class Rp2040FabricationRefusal extends Error {
  constructor(code, message) {
    super(message);
    this.name = "Rp2040FabricationRefusal";
    this.code = code;
  }
}

function refuse(code, message) {
  throw new Rp2040FabricationRefusal(code, message);
}

function canonicalConfiguration(configuration) {
  if (!configuration || typeof configuration !== "object" || Array.isArray(configuration)) {
    refuse("Configuration", "RP2040 fabrication configuration must be one finite object");
  }
  const keys = Object.keys(configuration).sort();
  const canonical = JSON.stringify(Object.fromEntries(keys.map((key) => [key, configuration[key]])));
  const bytes = encoder.encode(canonical);
  if (bytes.byteLength > MAXIMUM_CONFIGURATION_BYTES) {
    refuse("ConfigurationBound", `RP2040 fabrication configuration exceeds ${MAXIMUM_CONFIGURATION_BYTES} bytes`);
  }
  return { canonical, bytes };
}

function checkedSelection(selection) {
  if (!selection || typeof selection !== "object") refuse("Selection", "checked fabrication selection is required");
  for (const name of ["targetId", "profileId", "buildId", "imageId", "manifestPath"]) {
    if (typeof selection[name] !== "string" || selection[name].length < 1 || selection[name].length > 256) {
      refuse("Selection", `${name} is missing or exceeds its finite bound`);
    }
  }
  if (selection.targetId !== "conduit-target/rp2040-pico-w@1") {
    refuse("WrongTarget", "the RP2040 adapter cannot fabricate for another target");
  }
  return selection;
}

async function fetchReviewedTemplate(selection, fetchApi, cryptoApi) {
  const response = await fetchApi(selection.manifestPath, { cache: "no-store" });
  if (!response.ok) refuse("ManifestUnavailable", `reviewed artifact manifest is unavailable (${response.status})`);
  const manifest = await response.json();
  if (manifest.schema !== "conduit.creche/packaged-firmware-artifact@1"
    || manifest.target_id !== selection.targetId
    || manifest.build_identity !== selection.buildId
    || !Number.isSafeInteger(manifest.bytes)
    || manifest.bytes < UF2_BLOCK_BYTES
    || manifest.bytes > MAXIMUM_ARTIFACT_BYTES
    || manifest.maximum_bytes !== MAXIMUM_ARTIFACT_BYTES
    || !/^sha256:[0-9a-f]{64}$/.test(manifest.content_digest)
    || typeof manifest.path !== "string") {
    refuse("Manifest", "reviewed artifact manifest is malformed or outside the admitted bound");
  }
  const artifactResponse = await fetchApi(new URL(manifest.path, response.url), { cache: "no-store" });
  if (!artifactResponse.ok) refuse("ArtifactUnavailable", `reviewed artifact is unavailable (${artifactResponse.status})`);
  const bytes = new Uint8Array(await artifactResponse.arrayBuffer());
  if (bytes.byteLength !== manifest.bytes || bytes.byteLength > MAXIMUM_ARTIFACT_BYTES) {
    refuse("ArtifactBound", "reviewed artifact bytes do not match the admitted manifest bound");
  }
  const contentId = await sha256ContentId(bytes, cryptoApi);
  if (contentId !== manifest.content_digest) refuse("ArtifactDigest", "reviewed artifact digest does not match its manifest");
  parseRp2040Uf2(bytes, 4096);
  return { bytes, contentId, manifest };
}

function specializeTemplate(template, configurationBytes) {
  const blockCount = template.byteLength / UF2_BLOCK_BYTES;
  const last = new DataView(template.buffer, template.byteOffset + (blockCount - 1) * UF2_BLOCK_BYTES, UF2_BLOCK_BYTES);
  const nextAddress = last.getUint32(12, true) + UF2_PAYLOAD_BYTES;
  const bytes = new Uint8Array(template.byteLength + UF2_BLOCK_BYTES);
  bytes.set(template);
  for (let index = 0; index < blockCount; index += 1) {
    new DataView(bytes.buffer, index * UF2_BLOCK_BYTES, UF2_BLOCK_BYTES).setUint32(24, blockCount + 1, true);
  }
  const view = new DataView(bytes.buffer, blockCount * UF2_BLOCK_BYTES, UF2_BLOCK_BYTES);
  view.setUint32(0, 0x0a324655, true);
  view.setUint32(4, 0x9e5d5157, true);
  view.setUint32(8, 0x00002000, true);
  view.setUint32(12, nextAddress, true);
  view.setUint32(16, UF2_PAYLOAD_BYTES, true);
  view.setUint32(20, blockCount, true);
  view.setUint32(24, blockCount + 1, true);
  view.setUint32(28, 0xe48bff56, true);
  const payload = bytes.subarray(blockCount * UF2_BLOCK_BYTES + 32, blockCount * UF2_BLOCK_BYTES + 32 + UF2_PAYLOAD_BYTES);
  payload.set(encoder.encode("CONDUIT_CONFIG@1\n"));
  payload.set(configurationBytes, 17);
  view.setUint32(508, 0x0ab16f30, true);
  return bytes;
}

export async function bindRp2040BodySpore(imageBytes, prepared, cryptoApi = globalThis.crypto) {
  if (prepared?.output !== "uf2" || prepared?.target_id !== "conduitos/thumbv6m/pico-w") {
    refuse("SporeTarget", "prepared Body binding is not for the exact RP2040 Pico W target");
  }
  const provision = {
    protocol: 2,
    spore_id: prepared.spore_id,
    image_id: prepared.image_id,
    invitation_id: prepared.invitation_id,
    body_id: prepared.body_id,
    nonce: prepared.invitation_nonce,
    expires_at_millis: prepared.invitation_expires_at_millis,
    secret: prepared.invitation_secret,
  };
  const provisionBytes = encoder.encode(JSON.stringify(provision));
  if (provisionBytes.byteLength < 1
    || provisionBytes.byteLength > RP2040_UF2.sporeSectorBytes - SPORE_HEADER_BYTES) {
    refuse("SporeBound", "RP2040 Body bootstrap exceeds its exact reserved flash sector");
  }
  const parsed = parseRp2040Uf2(imageBytes, 4096);
  if (parsed.endAddress > RP2040_UF2.sporeSectorAddress) {
    refuse("SporeOverlap", "RP2040 IMAGE overlaps the reserved native Spore bootstrap sector");
  }
  const base = imageBytes instanceof Uint8Array ? imageBytes : new Uint8Array(imageBytes);
  const originalBlocks = base.byteLength / UF2_BLOCK_BYTES;
  const sporeBlocks = RP2040_UF2.sporeSectorBytes / UF2_PAYLOAD_BYTES;
  const totalBlocks = originalBlocks + sporeBlocks;
  const bytes = new Uint8Array(base.byteLength + sporeBlocks * UF2_BLOCK_BYTES);
  bytes.set(base);
  for (let index = 0; index < originalBlocks; index += 1) {
    new DataView(bytes.buffer, index * UF2_BLOCK_BYTES, UF2_BLOCK_BYTES).setUint32(24, totalBlocks, true);
  }
  const sector = new Uint8Array(RP2040_UF2.sporeSectorBytes).fill(0xff);
  sector.set(SPORE_MAGIC);
  new DataView(sector.buffer).setUint32(SPORE_MAGIC.byteLength, provisionBytes.byteLength, true);
  sector.set(provisionBytes, SPORE_HEADER_BYTES);
  for (let page = 0; page < sporeBlocks; page += 1) {
    const index = originalBlocks + page;
    const offset = index * UF2_BLOCK_BYTES;
    const view = new DataView(bytes.buffer, offset, UF2_BLOCK_BYTES);
    view.setUint32(0, 0x0a324655, true);
    view.setUint32(4, 0x9e5d5157, true);
    view.setUint32(8, 0x00002000, true);
    view.setUint32(12, RP2040_UF2.sporeSectorAddress + page * UF2_PAYLOAD_BYTES, true);
    view.setUint32(16, UF2_PAYLOAD_BYTES, true);
    view.setUint32(20, index, true);
    view.setUint32(24, totalBlocks, true);
    view.setUint32(28, RP2040_UF2.familyId, true);
    bytes.set(
      sector.subarray(page * UF2_PAYLOAD_BYTES, (page + 1) * UF2_PAYLOAD_BYTES),
      offset + 32,
    );
    view.setUint32(508, 0x0ab16f30, true);
  }
  parseRp2040Uf2(bytes, 4096);
  const recovered = readRp2040BodySpore(bytes);
  if (JSON.stringify(recovered) !== JSON.stringify(provision)) {
    refuse("SporeBinding", "RP2040 native Spore did not retain its exact Body bootstrap");
  }
  const contentId = await sha256ContentId(bytes, cryptoApi);
  return Object.freeze({
    schema: "conduit.rp2040/native-body-spore@1",
    format: "uf2",
    bytes,
    content_id: contentId,
    image_content_id: prepared.image_content_digest,
    spore_id: prepared.spore_id,
    bootstrap_bytes: provisionBytes.byteLength,
    bootstrap_flash_address: RP2040_UF2.sporeSectorAddress,
  });
}

export function readRp2040BodySpore(value) {
  const bytes = value instanceof Uint8Array ? value : new Uint8Array(value);
  parseRp2040Uf2(bytes, 4096);
  const sector = new Uint8Array(RP2040_UF2.sporeSectorBytes).fill(0xff);
  let pages = 0;
  for (let offset = 0; offset < bytes.byteLength; offset += UF2_BLOCK_BYTES) {
    const view = new DataView(bytes.buffer, bytes.byteOffset + offset, UF2_BLOCK_BYTES);
    const address = view.getUint32(12, true);
    if (address >= RP2040_UF2.sporeSectorAddress
      && address < RP2040_UF2.sporeSectorAddress + RP2040_UF2.sporeSectorBytes) {
      const page = (address - RP2040_UF2.sporeSectorAddress) / UF2_PAYLOAD_BYTES;
      sector.set(bytes.subarray(offset + 32, offset + 32 + UF2_PAYLOAD_BYTES), page * UF2_PAYLOAD_BYTES);
      pages += 1;
    }
  }
  if (pages !== RP2040_UF2.sporeSectorBytes / UF2_PAYLOAD_BYTES
    || SPORE_MAGIC.some((byte, index) => sector[index] !== byte)) {
    refuse("SporeMissing", "RP2040 UF2 omits its exact native Spore bootstrap sector");
  }
  const length = new DataView(sector.buffer).getUint32(SPORE_MAGIC.byteLength, true);
  if (length < 1 || length > RP2040_UF2.sporeSectorBytes - SPORE_HEADER_BYTES) {
    refuse("SporeBound", "RP2040 UF2 bootstrap length is outside its reserved sector");
  }
  try {
    const value = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(
      sector.subarray(SPORE_HEADER_BYTES, SPORE_HEADER_BYTES + length),
    ));
    if (value?.protocol !== 2 || typeof value.spore_id !== "string"
      || typeof value.image_id !== "string" || typeof value.invitation_id !== "string"
      || typeof value.body_id !== "string" || !Array.isArray(value.nonce)
      || !Array.isArray(value.secret)) {
      refuse("SporeMalformed", "RP2040 UF2 bootstrap has the wrong bounded provision shape");
    }
    return value;
  } catch (error) {
    if (error instanceof Rp2040FabricationRefusal) throw error;
    refuse("SporeMalformed", "RP2040 UF2 bootstrap is not exact JSON");
  }
}

export function createRp2040BrowserFabricationAdapter({ fetchApi = globalThis.fetch, cryptoApi = globalThis.crypto } = {}) {
  if (typeof fetchApi !== "function") refuse("FetchUnavailable", "reviewed artifact acquisition is unavailable");
  return Object.freeze({
    async fabricate(request) {
      const selection = checkedSelection(request?.selection);
      const strategy = request?.strategy;
      if (strategy !== "packaged-exact" && strategy !== "template-specialized") {
        refuse("UnsupportedStrategy", "no local RP2040 fabrication strategy matches the checked selection");
      }
      const configuration = canonicalConfiguration(request.configuration ?? {});
      const reviewed = await fetchReviewedTemplate(selection, fetchApi, cryptoApi);
      const bytes = strategy === "packaged-exact"
        ? reviewed.bytes
        : specializeTemplate(reviewed.bytes, configuration.bytes);
      if (bytes.byteLength > MAXIMUM_ARTIFACT_BYTES) refuse("ArtifactBound", "fabricated artifact exceeds its admitted byte bound");
      parseRp2040Uf2(bytes, 4096);
      const contentId = await sha256ContentId(bytes, cryptoApi);
      const binding = await sha256ContentId(encoder.encode(JSON.stringify({
        target_id: selection.targetId,
        profile_id: selection.profileId,
        build_id: selection.buildId,
        image_id: selection.imageId,
        strategy,
        artifact_content_id: contentId,
        configuration: configuration.canonical,
      })), cryptoApi);
      return Object.freeze({
        schema: "conduit.rp2040/browser-fabrication-result@1",
        strategy,
        bytes,
        content_id: contentId,
        selection_binding: binding,
        maximum_artifact_bytes: MAXIMUM_ARTIFACT_BYTES,
        provenance: Object.freeze({
          mechanism: strategy,
          artifact_id: reviewed.manifest.artifact_id,
          package_id: reviewed.manifest.artifact_id,
          template_content_id: reviewed.contentId,
          build_identity: reviewed.manifest.build_identity,
          source_revision: reviewed.manifest.source_revision,
          configuration: configuration.canonical,
          remote_builder: null,
          uploaded_artifact: null,
          cache_fallback: null,
        }),
      });
    },
  });
}

export const RP2040_BROWSER_FABRICATION = Object.freeze({
  strategies: Object.freeze(["packaged-exact", "template-specialized"]),
  maximumArtifactBytes: MAXIMUM_ARTIFACT_BYTES,
  maximumConfigurationBytes: MAXIMUM_CONFIGURATION_BYTES,
});
