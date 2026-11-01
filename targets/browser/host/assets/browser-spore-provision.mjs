// Parse inert owner-prepared provision only. This does not authenticate a
// release, admit a Host, select a Line, or start a Plot/Mask.
import { boundedRendezvousCandidates } from '../../workspace/creche-rendezvous-candidates.mjs';

const MAXIMUM_BYTES = 256 * 1024;
const encoder = new TextEncoder();
const decoder = new TextDecoder('utf-8', { fatal: true });
const fail = () => { throw new TypeError('Invalid bounded self-joining browser Spore provision'); };
const text = value => typeof value === 'string' && value.length > 0
  && encoder.encode(value).length <= 256;
const bytes32 = value => Array.isArray(value) && value.length === 32
  && value.every(byte => Number.isInteger(byte) && byte >= 0 && byte <= 255)
  && value.some(byte => byte !== 0);
const digest = value => typeof value === 'string' && /^sha256:[0-9a-f]{64}$/.test(value);

// Inert public provenance from the human-selected owner-produced artifact.
// It is never a substitute for independently reviewed executable policy.
export function parseBodyRootAuthority(value, expectedBodyId) {
  if (!value || value.schema !== 'conduit.body/root-authority@1'
    || Object.keys(value).sort().join(',') !== 'body_id,owner_host_id,public_key,schema'
    || !text(value.body_id) || value.body_id !== expectedBodyId
    || !text(value.owner_host_id) || !bytes32(value.public_key)
    || encoder.encode(JSON.stringify(value)).length > 1024) fail();
  return Object.freeze({ schema: value.schema, body_id: value.body_id,
    owner_host_id: value.owner_host_id, public_key: Object.freeze([...value.public_key]) });
}

export function parseBodyRootProducer(value, authority) {
  if (!authority || !value || Object.keys(value).sort().join(',') !== 'boot_id,host_id'
    || value.host_id !== authority.owner_host_id || !text(value.boot_id)) fail();
  return Object.freeze({ host_id: value.host_id, boot_id: value.boot_id });
}

// Accept bytes from the caller's bounded package reader, never an ambient URL.
// Reparse to detach all retained fields from caller-owned mutable objects.
export function parseBrowserSporeProvision(bytes, { nowMillis, expectedBodyId } = {}) {
  if (!(bytes instanceof Uint8Array) || bytes.length < 1 || bytes.length > MAXIMUM_BYTES
    || !Number.isSafeInteger(nowMillis) || nowMillis < 0
    || (expectedBodyId !== undefined && !text(expectedBodyId))) fail();
  let provision;
  try { provision = JSON.parse(decoder.decode(bytes)); } catch { fail(); }
  const manifest = provision?.spore;
  const invitation = provision?.invitation_provision;
  if (provision?.schema !== 'conduit.spore/native-package-provision@1'
    || manifest?.schema !== 'conduit.body/spore-manifest@2'
    || manifest.target !== 'browser/wasm32/page' || manifest.output !== 'browser-bundle'
    || manifest.binding?.mode !== 'self-joining'
    || !['spore_id', 'image_id', 'profile_id', 'body_id'].every(key => text(manifest[key]))
    || !digest(manifest.image_content_digest)
    || manifest.make?.make_package_id !== 'browser-wasm@1'
    || manifest.make.output !== manifest.output
    || !text(manifest.make.builder_adapter) || !text(manifest.make.deployment_adapter)
    || !text(invitation?.invitation_id)
    || manifest.binding.invitation_id !== invitation.invitation_id
    || (expectedBodyId !== undefined && manifest.body_id !== expectedBodyId)
    || !bytes32(invitation.nonce) || !bytes32(invitation.secret)
    || !Number.isSafeInteger(invitation.expires_at_millis)
    || invitation.expires_at_millis <= nowMillis) fail();

  const rendezvous = invitation.rendezvous ?? null;
  // Same opaque routed-invitation scope as the existing ZIP consumer. Actual
  // endpoint authentication and supported Line selection remain downstream.
  if (rendezvous !== null && (rendezvous.protocol !== 1
    || rendezvous.body_id !== manifest.body_id
    || rendezvous.invitation_id !== invitation.invitation_id
    || !Array.isArray(rendezvous.candidates)
    || rendezvous.candidates.length < 1 || rendezvous.candidates.length > 4
    || encoder.encode(JSON.stringify(rendezvous)).length > 16 * 1024)) fail();
  const candidates = boundedRendezvousCandidates(invitation.rendezvous_candidates, {
    bodyId: manifest.body_id, invitationExpiresAtMillis: invitation.expires_at_millis,
  });
  const rootAuthority = invitation.root_authority === undefined ? null
    : parseBodyRootAuthority(invitation.root_authority, manifest.body_id);
  return Object.freeze({
    manifest: freeze(manifest),
    rootAuthority,
    rootProducer: rootAuthority === null ? null
      : parseBodyRootProducer(invitation.root_producer, rootAuthority),
    claim: Object.freeze({ invitation_id: invitation.invitation_id,
      body_id: manifest.body_id, nonce: Object.freeze(invitation.nonce),
      expires_at_millis: invitation.expires_at_millis }),
    // Caller may erase this detached secret after handing it to the existing
    // membership ABI. Do not freeze it or retain a second invitation copy.
    secret: invitation.secret,
    rendezvous: freeze(rendezvous),
    rendezvousCandidates: candidates,
  });
}

function freeze(value) {
  if (value && typeof value === 'object') {
    for (const child of Object.values(value)) freeze(child);
    Object.freeze(value);
  }
  return value;
}
