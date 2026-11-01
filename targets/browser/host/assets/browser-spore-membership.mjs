import { openBrowserPartMembershipStore } from "./browser-part-membership.mjs";
import { parseBodyRootAuthority, parseBodyRootProducer } from "./browser-spore-provision.mjs";

// Invitation proof frames for the existing browser-window admission protocol.
// This module selects no route and grants no membership or execution authority.
export function createInvitedBrowserHandshake({ provision, expectedBodyId,
  hostId, bootId, advertisement, verifyingKey, membership, nowMillis = Date.now() }) {
  const claim = structuredClone(provision?.claim);
  const secret = provision?.secret;
  const bytes32 = value => Array.isArray(value) && value.length === 32
    && value.every(byte => Number.isInteger(byte) && byte >= 0 && byte <= 255)
    && value.some(byte => byte !== 0);
  const text = value => typeof value === 'string' && value.length > 0 && value.length <= 256;
  if (!claim || !text(claim.invitation_id) || !text(claim.body_id)
    || !text(expectedBodyId) || claim.body_id !== expectedBodyId
    || !bytes32(claim.nonce) || !bytes32(secret) || !bytes32(verifyingKey)
    || !Number.isSafeInteger(nowMillis) || nowMillis < 0
    || !Number.isSafeInteger(claim.expires_at_millis) || claim.expires_at_millis <= nowMillis
    || !text(hostId) || !text(bootId) || advertisement?.host_id !== hostId
    || advertisement?.boot_id !== bootId || typeof membership?.proveSpawn !== 'function') {
    throw new TypeError('Invalid invitation-bound browser handshake');
  }
  const retainedAdvertisement = structuredClone(advertisement);
  const retainedKey = [...verifyingKey];
  // Move the caller's erasable provision secret into one finite handshake copy.
  const retainedSecret = [...secret];
  secret.fill(0);
  let phase = 'ready';
  return Object.freeze({
    advertise() {
      if (phase !== 'ready') throw new Error('Invitation handshake cannot be replayed');
      phase = 'advertised';
      return { kind: 'invited-advertise', protocol: 1,
        advertisement: structuredClone(retainedAdvertisement),
        claim: structuredClone(claim), verifying_key: [...retainedKey] };
    },
    prove(challenge) {
      if (phase !== 'advertised') throw new Error('Invitation proof has no current advertisement');
      // Never sign a different invitation, including one with a changed expiry.
      const matches = challenge?.invitation_id === claim.invitation_id
        && challenge.body_id === claim.body_id && challenge.expires_at_millis === claim.expires_at_millis
        && bytes32(challenge.nonce) && challenge.nonce.every((byte, i) => byte === claim.nonce[i]);
      if (!matches) {
        phase = 'retired'; retainedSecret.fill(0);
        throw new Error('Invitation challenge differs from retained Spore');
      }
      phase = 'retired';
      try {
        const proof = membership.proveSpawn(structuredClone(claim), retainedSecret);
        if (proof.hostId !== hostId || proof.bootId !== bootId
          || !Array.isArray(proof.signature) || proof.signature.length !== 64
          || !proof.signature.every(byte => Number.isInteger(byte) && byte >= 0 && byte <= 255)) {
          throw new Error('Invitation proof differs from admitted browser incarnation');
        }
        return { kind: 'spawn-proof', protocol: 1, invitation_id: claim.invitation_id,
          body_id: claim.body_id, host_id: hostId, boot_id: bootId,
          nonce: [...claim.nonce], signature: [...proof.signature] };
      } finally { retainedSecret.fill(0); }
    },
    retire() { phase = 'retired'; retainedSecret.fill(0); },
  });
}

// Explicit composition for a human-selected owner Spore. The runtime remains
// the only receipt verifier; selected storage retains its private Part record.
// This adapter neither authenticates a route nor authorizes presence/Face use.
export async function openVerifiedBrowserSporeMembership({ membership, storage,
  expectedBodyId, sporeRoot, invitedProvision, hostId, bootId, advertisement,
  retainedCredential = null }) {
  const authority = parseBodyRootAuthority(sporeRoot?.rootAuthority, expectedBodyId);
  const producer = parseBodyRootProducer(sporeRoot?.rootProducer, authority);
  if (typeof expectedBodyId !== 'string' || authority.body_id !== expectedBodyId
    || typeof membership?.prepareMembershipScope !== 'function'
    || typeof membership?.adoptMembershipScope !== 'function'
    || membership.hostId !== hostId || membership.bootId !== bootId) {
    throw new TypeError('Root-aware membership requires this admitted Host and selected Body');
  }
  const store = await openBrowserPartMembershipStore({ membership, storage,
    expectedBodyId, expectedRoot: authority.public_key });
  // Import checks the persistent Host and exact Root before any new preparation.
  const loaded = await store.load();
  let phase = loaded === null ? 'unprepared' : 'verified';
  const samePart = credential => credential?.body_id === expectedBodyId
    && credential.host_id === hostId
    && credential.part_id === loaded?.admissionCredential.part_id;
  let priorCredential = loaded?.admissionCredential ?? null;
  if (retainedCredential !== null) {
    if (!loaded || !samePart(retainedCredential)) {
      throw new Error('Retained credential has no matching verified Part custody');
    }
    priorCredential = retainedCredential;
  }
  if (loaded !== null) {
    invitedProvision?.secret?.fill?.(0);
  } else {
    if (!invitedProvision) throw new Error('First Root-aware admission requires the selected invitation');
    try {
      const request = membership.prepareMembershipScope({ claim: invitedProvision.claim,
        secret: invitedProvision.secret, expectedBodyId, expectedRoot: [...authority.public_key],
        producerHost: producer.host_id, producerBoot: producer.boot_id });
      phase = 'pending';
      if (request.body_id !== expectedBodyId || request.host_id !== hostId
        || request.boot_id !== bootId || request.offer_generation !== advertisement?.offer_generation
        || request.invitation_id !== invitedProvision.claim.invitation_id
        || !Array.isArray(request.nonce) || request.nonce.length !== 32
        || !request.nonce.every((byte, i) => byte === invitedProvision.claim.nonce[i])) {
        throw new Error('Prepared membership request differs from the actual offered incarnation');
      }
    } catch (error) {
      if (phase === 'pending') membership.retireMembershipScope({ expectedBodyId });
      invitedProvision.secret?.fill?.(0);
      throw error;
    }
  }
  return Object.freeze({
    priorCredential,
    returning: loaded !== null,
    async accept(receipt, offeredCredential = null) {
      if (phase !== 'pending') throw new Error('No pending signed membership receipt');
      if (!receipt) throw new Error('Selected Root requires a signed membership receipt');
      const credential = membership.adoptMembershipScope(receipt);
      // From this point transport failure or storage pressure must not erase
      // the only verified Part signer. The owner may already have consumed it.
      phase = 'verified';
      if (credential?.body_id !== expectedBodyId || credential.host_id !== hostId
        || credential.boot_id !== bootId || (offeredCredential !== null
          && ['credential_id', 'body_id', 'part_id', 'host_id', 'boot_id', 'issued_at_millis']
            .some(field => credential[field] !== offeredCredential[field]))) {
        throw new Error('Signed membership credential differs from this admission');
      }
      await store.retain();
      return Object.freeze({ ...credential });
    },
    cancelPending() {
      if (phase === 'pending') {
        membership.retireMembershipScope({ expectedBodyId });
        phase = 'retired';
      }
    },
  });
}
