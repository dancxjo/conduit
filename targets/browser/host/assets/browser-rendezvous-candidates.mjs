const MAXIMUM_CANDIDATES = 4;
const MAXIMUM_TEXT_BYTES = 512;
const encoder = new TextEncoder();

export function boundedRendezvousCandidates(value, { bodyId, invitationExpiresAtMillis }) {
  if (value === undefined) return Object.freeze([]);
  if (!Array.isArray(value) || value.length > MAXIMUM_CANDIDATES) {
    throw new TypeError("rendezvous candidates must be one finite ordered list");
  }
  return Object.freeze(value.map((candidate) => boundedCandidate(candidate, {
    bodyId,
    invitationExpiresAtMillis,
  })));
}

function boundedCandidate(candidate, { bodyId, invitationExpiresAtMillis }) {
  if (candidate?.schema !== "conduit.body/rendezvous-candidate@1"
    || candidate.body_id !== bodyId
    || !boundedText(candidate.line_family)
    || !boundedText(candidate.locator)
    || !boundedText(candidate.rendezvous_identity)
    || !Number.isSafeInteger(candidate.expires_at_millis)
    || candidate.expires_at_millis < 0
    || candidate.expires_at_millis > invitationExpiresAtMillis
    || !Number.isSafeInteger(candidate.maximum_attempts)
    || candidate.maximum_attempts < 1
    || candidate.maximum_attempts > 8
    || !Number.isSafeInteger(candidate.connection_timeout_millis)
    || candidate.connection_timeout_millis < 1
    || candidate.connection_timeout_millis > 60_000) {
    throw new TypeError("rendezvous candidate is incomplete or outside its finite authority bounds");
  }
  const authentication = candidate.authentication;
  const authenticationKeys = authentication && typeof authentication === "object"
    ? Object.keys(authentication)
    : [];
  if (!authentication
    || !["mutual-tls", "conduit-authenticated-line"].includes(authentication.mode)
    || !boundedText(authentication.server_identity)
    || (authentication.credential_reference !== undefined
      && !boundedText(authentication.credential_reference))
    || authenticationKeys.some((key) => !["mode", "server_identity", "credential_reference"].includes(key))) {
    throw new TypeError("non-loopback rendezvous candidate lacks bounded endpoint authentication");
  }
  return Object.freeze({
    schema: candidate.schema,
    line_family: candidate.line_family,
    locator: candidate.locator,
    body_id: candidate.body_id,
    rendezvous_identity: candidate.rendezvous_identity,
    expires_at_millis: candidate.expires_at_millis,
    maximum_attempts: candidate.maximum_attempts,
    connection_timeout_millis: candidate.connection_timeout_millis,
    authentication: Object.freeze({
      mode: authentication.mode,
      server_identity: authentication.server_identity,
      ...(authentication.credential_reference === undefined
        ? {}
        : { credential_reference: authentication.credential_reference }),
    }),
  });
}

function boundedText(value) {
  return typeof value === "string" && value.length > 0
    && encoder.encode(value).byteLength <= MAXIMUM_TEXT_BYTES;
}
