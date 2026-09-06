const MAXIMUM_IMAGE_BYTES = 80 * 1024 * 1024;

export async function acquireConduitOsRelease(profile, signal) {
  let response;
  try {
    response = await fetch(profile.manifestPath, { signal, cache: "no-store" });
  } catch (error) {
    refuse("ArtifactUnavailable", "reviewed ConduitOS release manifest is unavailable", error);
  }
  if (!response.ok) refuse("ArtifactUnavailable", `reviewed ConduitOS release manifest returned HTTP ${response.status}`);
  const manifest = await response.json();
  validateConduitOsReleaseManifest(manifest, profile);
  let artifactResponse;
  try {
    artifactResponse = await fetch(new URL(manifest.artifact.path, response.url), { signal, cache: "no-store" });
  } catch (error) {
    refuse("ArtifactUnavailable", "reviewed ConduitOS disk IMAGE is unavailable", error);
  }
  if (!artifactResponse.ok) refuse("ArtifactUnavailable", `reviewed ConduitOS disk IMAGE returned HTTP ${artifactResponse.status}`);
  const bytes = new Uint8Array(await artifactResponse.arrayBuffer());
  if (bytes.byteLength !== manifest.artifact.bytes || bytes.byteLength < 1 || bytes.byteLength > MAXIMUM_IMAGE_BYTES) {
    refuse("StaleArtifact", "reviewed ConduitOS disk IMAGE violated its exact finite byte bound");
  }
  const digest = await sha256(bytes);
  if (digest !== manifest.artifact.sha256 || manifest.image_id !== `image:${digest}`) {
    refuse("StaleArtifact", "reviewed ConduitOS disk IMAGE content identity is stale");
  }
  return Object.freeze({ manifest: Object.freeze(manifest), bytes, digest });
}

export function validateConduitOsReleaseManifest(manifest, profile) {
  if (manifest?.target_id !== profile.target.id) refuse("WrongMachine", "reviewed ConduitOS release names a different exact target");
  if (manifest?.architecture !== profile.architecture) refuse("WrongArchitecture", "reviewed ConduitOS release names a different architecture");
  if (manifest?.machine !== profile.machine) refuse("WrongMachine", "reviewed ConduitOS release names a different machine");
  if (manifest?.artifact_role !== "product-host") refuse("UnsupportedProductRole", "architecture proof appliances are not Crèche product Host artifacts");
  if (typeof manifest?.boot_assets?.firmware !== "string" || manifest.boot_assets.firmware.length === 0) {
    refuse("MissingFirmware", "reviewed ConduitOS release omitted its required firmware identity");
  }
  if (typeof manifest?.boot_assets?.boot_entry !== "string" || manifest.boot_assets.boot_entry.length === 0) {
    refuse("MissingBootloader", "reviewed ConduitOS release omitted its required boot entry identity");
  }
  if (manifest.schema !== "conduit.conduitos/creche-release@1"
    || manifest.fabrication_package_id !== "conduitos-image@1"
    || manifest.output !== "disk-image"
    || manifest.builder_adapter !== profile.builderAdapter
    || manifest.deployment_adapter !== profile.deploymentAdapter
    || manifest.boot_mechanism !== profile.bootMechanism
    || manifest.boot_assets.firmware !== profile.firmware
    || manifest.boot_assets.boot_entry !== profile.bootEntry
    || manifest.artifact?.role !== "final-bootable-image"
    || manifest.artifact?.format !== "hybrid-iso"
    || typeof manifest.artifact?.path !== "string" || manifest.artifact.path.includes("/")
    || !Number.isSafeInteger(manifest.artifact?.bytes) || manifest.artifact.bytes < 1 || manifest.artifact.bytes > MAXIMUM_IMAGE_BYTES
    || !/^sha256:[0-9a-f]{64}$/.test(manifest.artifact?.sha256)
    || manifest.image_id !== `image:${manifest.artifact.sha256}`
    || typeof manifest.profile_id !== "string" || typeof manifest.build_id !== "string"
    || typeof manifest.source_identity !== "string" || typeof manifest.toolchain_identity !== "string"
    || JSON.stringify(manifest.expected_offers) !== JSON.stringify(["conduit.host/present@1"])
    || JSON.stringify(manifest.bounds) !== JSON.stringify(profile.bounds)
    || manifest.boot_claimed !== false || manifest.physical_proof_claimed !== false) {
    refuse("StaleArtifact", "reviewed ConduitOS release contract or identity is stale");
  }
  return Object.freeze({ ...manifest });
}

export function validateLoaderEvidence(evidence, profile, binding) {
  if (!evidence || typeof evidence.loader_id !== "string") refuse("UnavailableWriter", "no explicit local disk or VM loader receipt was supplied");
  if (evidence.target_id !== profile.target.id || evidence.machine !== profile.machine) refuse("WrongMachine", "local loader receipt names a different machine");
  if (evidence.architecture !== profile.architecture) refuse("WrongArchitecture", "local loader receipt names a different architecture");
  if (evidence.image_content_digest !== binding.prepared?.image_content_digest
    || evidence.artifact_sha256 !== binding.nativeSpore?.content_digest
    || evidence.artifact_bytes !== binding.nativeSpore?.bytes?.byteLength) refuse("StaleArtifact", "local loader receipt names a different Body-bound ISO identity");
  if (!profile.supportedCarriers.includes(evidence.carrier) || evidence.explicit_authority !== true || evidence.load_completed !== true) {
    refuse("LoaderEvidenceInvalid", "local loader receipt omitted a supported carrier, explicit authority, or completed load truth");
  }
  return Object.freeze({ ...evidence });
}

async function sha256(bytes) {
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", bytes));
  return `sha256:${Array.from(digest, (byte) => byte.toString(16).padStart(2, "0")).join("")}`;
}

function refuse(code, message, cause) {
  const error = new Error(message, cause ? { cause } : undefined);
  error.name = "ConduitOsProvisioningRefusal";
  error.code = code;
  error.evidence = Object.freeze({
    schema: "conduit.conduitos/creche-provisioning-refusal@1",
    terminal: code,
    message,
    browser_device_authority_requested: false,
    external_work_started: false,
  });
  throw error;
}
