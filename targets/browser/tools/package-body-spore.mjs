// Compose existing reviewed Host-release and Body-bound ZIP machinery. The
// caller supplies an owner-prepared Spore; fabrication never mints authority.
import { isDeepStrictEqual } from 'node:util';
import { acquireHostRelease } from '../workspace/creche-release-bundle.mjs';
import { createBodyBoundZip, readBodyBoundZip } from '../workspace/creche-native-zip.mjs';

const digest = value => typeof value === 'string' && /^sha256:[0-9a-f]{64}$/.test(value);
const byteArray = value => Array.isArray(value) && value.length === 32
  && value.every(byte => Number.isInteger(byte) && byte >= 0 && byte <= 255)
  && value.some(byte => byte !== 0);

export async function packageBrowserBodySpore({ manifest, prepared, releaseUrl, fetcher, filename, nowMillis = Date.now() }) {
  if (manifest?.schema !== 'conduit.body/spore-manifest@2'
    || manifest.target !== 'browser/wasm32/page'
    || manifest.output !== 'browser-bundle' || manifest.binding?.mode !== 'self-joining'
    || manifest.make?.make_package_id !== 'browser-wasm@1'
    || manifest.make.output !== manifest.output || !digest(manifest.image_content_digest)) {
    throw new TypeError('A checked self-joining browser Spore is required');
  }
  const bound = prepared?.spore_manifest;
  if (!bound || !isDeepStrictEqual(bound, manifest)
    || prepared.spore_id !== manifest.spore_id || prepared.body_id !== manifest.body_id
    || prepared.image_content_digest !== manifest.image_content_digest
    || prepared.invitation_id !== manifest.binding.invitation_id
    || !byteArray(prepared.invitation_secret) || !byteArray(prepared.invitation_nonce)
    || !Number.isSafeInteger(nowMillis) || nowMillis < 0
    || !Number.isSafeInteger(prepared.invitation_expires_at_millis)
    || prepared.invitation_expires_at_millis <= nowMillis) {
    throw new TypeError('The owner-prepared invitation must match the exact checked Spore');
  }
  const release = await acquireHostRelease({
    target_id: 'browser/wasm32/page', package_id: manifest.make.make_package_id,
    output: manifest.output, builder_adapter: manifest.make.builder_adapter,
    deployment_adapter: manifest.make.deployment_adapter, manifest_path: String(releaseUrl),
  }, undefined, { manifestUrl: releaseUrl, fetcher });
  if (release.manifest.bundle_sha256 !== manifest.image_content_digest) {
    throw new TypeError('The checked Spore does not bind the exact reviewed browser release');
  }
  const artifact = await createBodyBoundZip({ prepared, release, filename });
  // Apply the existing consumer's checks before publishing any artifact.
  readBodyBoundZip(artifact.bytes);
  return artifact;
}
