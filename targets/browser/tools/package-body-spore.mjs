// Compose existing reviewed Host-release and Body-bound ZIP machinery. The
// caller supplies an owner-prepared Spore; fabrication never mints authority.
import { isDeepStrictEqual } from 'node:util';
import { acquireHostRelease } from '../workspace/creche-release-bundle.mjs';
import { createBodyBoundZip, readBodyBoundZip } from '../workspace/creche-native-zip.mjs';
import { parseBodyRootAuthority } from '../host/assets/browser-spore-provision.mjs';

const digest = value => typeof value === 'string' && /^sha256:[0-9a-f]{64}$/.test(value);
const byteArray = value => Array.isArray(value) && value.length === 32
  && value.every(byte => Number.isInteger(byte) && byte >= 0 && byte <= 255)
  && value.some(byte => byte !== 0);

export async function packageBrowserBodySpore({ manifest, prepared, releaseUrl, fetcher, filename, nowMillis = Date.now() }) {
  manifest = structuredClone(manifest);
  prepared = structuredClone(prepared);
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
  parseBodyRootAuthority(prepared.root_authority, manifest.body_id);
  const release = await acquireHostRelease({
    target_id: 'browser/wasm32/page', package_id: manifest.make.make_package_id,
    output: manifest.output, builder_adapter: manifest.make.builder_adapter,
    deployment_adapter: manifest.make.deployment_adapter, manifest_path: String(releaseUrl),
  }, undefined, { manifestUrl: releaseUrl, fetcher });
  if (release.manifest.bundle_sha256 !== manifest.image_content_digest) {
    throw new TypeError('The checked Spore does not bind the exact reviewed browser release');
  }
  if (typeof release.manifest.browser_image_id !== 'string'
    || release.manifest.browser_profile_id !== manifest.profile_id
    || !['runtime.wasm', 'conduit-browser-image.json', 'browser-boot-profile.mjs'].every(name => release.payloads.some(file => file.path === name))) {
    throw new TypeError('A configured BrowserBundle with the exact IMAGE and Profile is required');
  }
  const artifact = await createBodyBoundZip({ prepared, release, filename });
  // Apply the existing consumer's checks before publishing any artifact.
  readBodyBoundZip(artifact.bytes);
  return artifact;
}

// Filesystem adapter for the ordinary make command. Inputs remain private;
// publication is exclusive and contains no secret-bearing diagnostic output.
export async function publishBrowserBodySpore({ manifestPath, preparedPath, releasePath, outputPath, nowMillis }) {
  const { readFile, lstat, open, link, unlink } = await import('node:fs/promises');
  const path = await import('node:path');
  const { pathToFileURL, fileURLToPath } = await import('node:url');
  const readBounded = async (file, maximum) => {
    const stat = await lstat(file);
    if (!stat.isFile() || stat.size < 1 || stat.size > maximum) throw new TypeError('Package input exceeds its finite regular-file bound');
    return new Uint8Array(await readFile(file));
  };
  const parse = async file => JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(await readBounded(file, 256 * 1024)));
  const releaseRoot = path.dirname(path.resolve(releasePath));
  const releaseUrl = pathToFileURL(path.resolve(releasePath));
  const artifact = await packageBrowserBodySpore({
    manifest: await parse(manifestPath), prepared: await parse(preparedPath), releaseUrl,
    filename: path.basename(outputPath), nowMillis,
    fetcher: async url => {
      const file = fileURLToPath(url);
      if (path.dirname(file) !== releaseRoot) throw new TypeError('Reviewed release escaped its selected directory');
      return new Response(await readBounded(file, String(url) === String(releaseUrl) ? 256 * 1024 : 32 * 1024 * 1024));
    },
  });
  const staged = `${outputPath}.staging-${process.pid}`;
  const handle = await open(staged, 'wx', 0o600);
  try {
    await handle.writeFile(artifact.bytes);
    await handle.sync();
    await handle.close();
    await link(staged, outputPath); // never replace an existing private package
  } finally {
    await handle.close();
    await unlink(staged);
  }
  return { content_digest: artifact.content_digest, bytes: artifact.bytes.length,
    membership_admitted: false, host_boot_observed: false };
}
