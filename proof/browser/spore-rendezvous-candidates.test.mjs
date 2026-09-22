import assert from "node:assert/strict";
import test from "node:test";
import { boundedRendezvousCandidates } from "../../products/creche/browser/creche-rendezvous-candidates.mjs";
import { createBodyBoundZip, readBodyBoundZip } from "../../products/creche/browser/creche-native-zip.mjs";

const expiry = 2_000_000_000_000;
const candidate = Object.freeze({
  schema: "conduit.body/rendezvous-candidate@1",
  line_family: "conduit-line/mutual-tls@1",
  locator: "dns:body.example.test:443",
  body_id: "body/roseau",
  rendezvous_identity: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  expires_at_millis: expiry,
  maximum_attempts: 3,
  connection_timeout_millis: 5_000,
  authentication: Object.freeze({
    mode: "mutual-tls",
    server_identity: "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
  }),
});

test("body-bound native package carries finite authenticated rendezvous candidates", async () => {
  const prepared = {
    spore_id: "spore/one",
    image_id: "image/one",
    image_content_digest: "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
    body_id: "body/roseau",
    invitation_id: "invitation/one",
    invitation_nonce: Array(32).fill(1),
    invitation_secret: Array(32).fill(2),
    invitation_expires_at_millis: expiry,
    rendezvous_candidates: [candidate],
    spore_manifest: {
      schema: "conduit.body/spore-manifest@2",
      spore_id: "spore/one",
      body_id: "body/roseau",
      image_content_digest: "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
      binding: { mode: "self-joining", invitation_id: "invitation/one" },
    },
  };
  const packaged = await createBodyBoundZip({
    prepared,
    release: { payloads: [{ path: "conduit", bytes: new Uint8Array([1]), media_type: "application/octet-stream" }] },
    filename: "conduit-host.zip",
  });
  const recovered = readBodyBoundZip(packaged.bytes).provision.invitation_provision;
  assert.deepEqual(recovered.rendezvous_candidates, [candidate]);
});

test("candidate authority cannot outlive its invitation or omit endpoint authentication", () => {
  assert.throws(() => boundedRendezvousCandidates([
    { ...candidate, expires_at_millis: expiry + 1 },
  ], { bodyId: "body/roseau", invitationExpiresAtMillis: expiry }), /authority bounds/);
  assert.throws(() => boundedRendezvousCandidates([
    { ...candidate, authentication: { mode: "bearer", server_identity: "somewhere" } },
  ], { bodyId: "body/roseau", invitationExpiresAtMillis: expiry }), /endpoint authentication/);
  assert.throws(() => boundedRendezvousCandidates([
    { ...candidate, authentication: { ...candidate.authentication, shell: "ssh root@somewhere" } },
  ], { bodyId: "body/roseau", invitationExpiresAtMillis: expiry }), /endpoint authentication/);
});

test("candidate sets and retry policy remain finite", () => {
  assert.throws(() => boundedRendezvousCandidates(Array(5).fill(candidate), {
    bodyId: "body/roseau",
    invitationExpiresAtMillis: expiry,
  }), /finite ordered list/);
  assert.throws(() => boundedRendezvousCandidates([{ ...candidate, maximum_attempts: 9 }], {
    bodyId: "body/roseau",
    invitationExpiresAtMillis: expiry,
  }), /authority bounds/);
});
