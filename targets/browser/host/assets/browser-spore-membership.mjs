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
