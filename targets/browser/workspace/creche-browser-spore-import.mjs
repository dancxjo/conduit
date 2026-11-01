// Inert import for the existing Workspace entrance. This admits release bytes,
// not membership, a Line, a Plan or a Face. Policy must come from the selected
// reviewed release, independently of the human-supplied ZIP.
import { readBodyBoundZip } from './creche-native-zip.mjs';
import { parseBrowserSporeProvision } from '../host/assets/browser-spore-provision.mjs';
import { admitBrowserAssetMap } from '../sdk/browser-sdk.mjs';

export async function importBrowserSpore(bytes, { reviewedRelease, nowMillis, expectedBodyId } = {}) {
  const archive = readBodyBoundZip(bytes);
  const provisionBytes = archive.entries.get('conduit-spore.json');
  let provision;
  try {
    provision = parseBrowserSporeProvision(provisionBytes, { nowMillis, expectedBodyId });
    archive.entries.delete('conduit-spore.json');
    const admitted = await admitBrowserAssetMap(archive.entries, reviewedRelease, provision.manifest.profile_id);
    if (provision.manifest.image_content_digest !== admitted.bundle.bundle_sha256
      || provision.manifest.image_id !== admitted.image.image_id) {
      throw new TypeError('Browser Spore differs from the independently selected reviewed IMAGE');
    }
    return Object.freeze({ provision,
      browserOptions: Object.freeze({ assetMap: admitted.payloads,
        reviewedRelease: Object.freeze({ package: admitted.pkg, distribution: admitted.distribution, release: admitted.bundle }),
        profileId: admitted.pkg.profile_id }),
    });
  } catch (error) {
    provision?.secret.fill(0);
    throw error;
  } finally {
    provisionBytes.fill(0);
    // readBodyBoundZip's parsed copy is private and must not retain a second
    // invitation signing seed after the provision parser detaches its result.
    archive.provision.invitation_provision?.secret?.fill(0);
  }
}
