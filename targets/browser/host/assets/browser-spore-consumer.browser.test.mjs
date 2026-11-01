import test from 'node:test';
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { spawn } from 'node:child_process';
const chromium = process.env.CONDUIT_TEST_CHROMIUM;
// Executes the real consumer modules in Chromium with an explicitly stubbed
// membership verifier. This is neither WASM Boot nor live joined-Body proof.
test('browser signed receipt consumer retains custody without restoring presence', { skip: !chromium }, async () => {
  const script = `
import { joinBrowserBody } from '/host/assets/browser-membership.js';
const turn=()=>new Promise(resolve=>setTimeout(resolve,0));
try {
 const events=[],hostId='host/test',bootId='boot/test';
 const credential={credential_id:'credential/test',body_id:'body/test',part_id:'part/test',host_id:hostId,boot_id:bootId,issued_at_millis:1};
 const claim={invitation_id:'invitation/test',body_id:'body/test',nonce:Array(32).fill(3),expires_at_millis:Date.now()+60000};
 const api={memory:{buffer:new ArrayBuffer(8192)}};
 for(const name of ['input_ptr','output_ptr','output_len','initialize','prove','prove_return','advertisement'])api['conduit_browser_membership_'+name]=()=>0;
 api.conduit_browser_membership_input_capacity=()=>4096;api.conduit_browser_membership_output_capacity=()=>4096;
 let stored=null,receive,closed=false;
 const carrier={sendSessionFrame:async()=>{},receiveSessionFrame:()=>new Promise(resolve=>receive=resolve),close:()=>closed=true};
 const membership={schema:'conduit.browser/body-membership-client@1',hostId,bootId,verifyingKey:Array(32).fill(1),
 advertisement:()=>({host_id:hostId,boot_id:bootId,offer_generation:1}),
 prepareMembershipScope:()=>({...claim,host_id:hostId,boot_id:bootId,offer_generation:1}),
 proveSpawn:()=>({hostId,bootId,signature:Array(64).fill(4)}),
 adoptMembershipScope:receipt=>{events.push('verify');if(receipt.signature!=='valid')throw Error('signature');return credential;},
 exportPartMembership:()=>new Uint8Array([7]),importPartMembership:()=>({...credential,boot_id:'prior'}),retireMembershipScope:()=>events.push('retire')};
 const selectedStorage={readBytes:async()=>stored,writeBytes:async(key,bytes)=>{events.push('write');stored=bytes.slice();},deleteJson:async()=>{stored=null;}};
 const joined=await joinBrowserBody({sessionCarrier:carrier,wasmBytes:new Uint8Array([1]),admittedHost:{api,membership,hostId,bootId},
 expectedBodyId:'body/test',invitedProvision:{claim,secret:Array(32).fill(2)},selectedStorage,
 sporeRoot:{rootAuthority:{schema:'conduit.body/root-authority@1',body_id:'body/test',owner_host_id:'owner/test',public_key:Array(32).fill(11)},rootProducer:{host_id:'owner/test',boot_id:'owner-boot/test'}},
 onCredential:c=>{if(c.credential_id!==credential.credential_id)throw Error('credential');events.push('callback');},renewPresence:false,reconnectPresence:false});
 await turn();receive(new TextEncoder().encode(JSON.stringify({kind:'membership-receipt',protocol:1,receipt:{signature:'valid'}})));await turn();await turn();
 if(JSON.stringify(events)!==JSON.stringify(['verify','write','callback'])||!stored||!closed||joined.state()!=='custody-retained-awaiting-fresh-boot'||joined.presenceState()!=='unavailable'||joined.membershipCredential()!==null)throw Error('custody/authority ordering '+JSON.stringify(events));
 document.body.textContent='PASS';
} catch(e) { document.body.textContent='FAIL: '+e.stack; }
`;
  const root = new URL('../../', import.meta.url);
  const server = createServer(async (req, res) => {
    if (req.url === '/') { res.setHeader('Content-Type', 'text/html'); res.end(`<!doctype html><body>RUNNING<script type="module">${script}</script>`); return; }
    try {
      const url = new URL(`.${req.url}`, root);
      if (!url.href.startsWith(root.href) || !/\.(mjs|js)$/.test(url.pathname)) throw Error('path');
      res.setHeader('Content-Type', 'text/javascript'); res.end(await readFile(url));
    } catch { res.statusCode = 404; res.end(); }
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  try {
    const child = spawn(chromium, ['--no-sandbox', '--disable-gpu', '--dump-dom', '--timeout=10000', '--virtual-time-budget=3000', `http://127.0.0.1:${server.address().port}/`]);
    let output = '', errors = '';
    child.stdout.on('data', d => output += d); child.stderr.on('data', d => errors += d);
    const status = await new Promise((resolve, reject) => { child.on('error', reject); child.on('exit', resolve); });
    assert.equal(status, 0, errors); assert.match(output, /<body>PASS<\/body>/, output);
  } finally { await new Promise(resolve => server.close(resolve)); }
});
