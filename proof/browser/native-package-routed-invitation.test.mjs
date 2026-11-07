import assert from "node:assert/strict";
import test from "node:test";
import { createBodyBoundZip, readBodyBoundZip } from "../../targets/browser/workspace/creche-native-zip.mjs";
const descriptor = {
  protocol: 1, body_id: "body/test", invitation_id: "invite/test",
  candidates: [{ candidate_id: "route/test", line_family: "authenticated-conduit-line", reachability: "relay/test",
    authentication: { server_identity: "server/test", transport_binding_sha256: Array(32).fill(22) },
    expires_at_millis: 9999, maximum_attempts: 2, attempt_timeout_millis: 4000 }],
};
function prepared(rendezvous = descriptor) {
  const digest = `sha256:${"3".repeat(64)}`;
  return { spore_id:"spore/test", image_content_digest:digest, body_id:"body/test", invitation_id:"invite/test", invitation_nonce:Array(32).fill(3), invitation_secret:Array(32).fill(7), invitation_expires_at_millis:10000, rendezvous,
    spore_manifest:{schema:"conduit.body/spore-manifest@2",spore_id:"spore/test",body_id:"body/test",image_content_digest:digest,binding:{mode:"self-joining",invitation_id:"invite/test"}} };
}
const packageFor = (value) => createBodyBoundZip({prepared:value,release:{payloads:[{path:"conduit",bytes:new Uint8Array([1,2,3])}]},filename:"host.zip"});
test("native package preserves v2 descriptor and transport binding separately from legacy candidates", async () => {
  const result=readBodyBoundZip((await packageFor(prepared())).bytes);
  assert.deepEqual(result.provision.invitation_provision.rendezvous,descriptor);
  assert.deepEqual(result.provision.invitation_provision.rendezvous_candidates,[]);
  assert.equal(result.provision.invitation_provision.secret[0],7);
});
test("foreign or unauthenticated v2 descriptor is refused and v1 remains readable",async()=>{
  for (const change of [(d)=>{d.body_id="body/foreign";},(d)=>{d.candidates[0].authentication.transport_binding_sha256.fill(0);},(d)=>{d.candidates[0].expires_at_millis=10001;}]) {
    const value=structuredClone(descriptor);change(value);
    await assert.rejects(()=>packageFor(prepared(value)),/routed invitation/);
  }
  const result=readBodyBoundZip((await packageFor(prepared(null))).bytes);
  assert.equal(result.provision.invitation_provision.rendezvous,null);
});
