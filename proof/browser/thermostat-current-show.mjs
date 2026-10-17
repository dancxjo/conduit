// Opt-in Chromium proof of local SDK Show binding against the public runtime.
import { chromium, expect } from '@playwright/test';
import { readFile, writeFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
const receiptPath = process.argv[2];
if (!receiptPath) throw new Error('an explicit receipt output path is required');
const hash = s => 'sha256:' + createHash('sha256').update(s).digest('hex');
const browser = await chromium.launch({headless:true});
try {
 const context=await browser.newContext(),page=await context.newPage(),errors=[];
 page.on('pageerror',e=>errors.push(e.message));
 const root='https://dancxjo.github.io/conduit/handbook/';
 let sdk=await (await context.request.get(root+'sdk/browser-sdk.mjs')).text();
 sdk=sdk.replace('import { checkedOwnerFaceArguments }','import { checkedOwnerFaceArguments, checkedAcknowledgedOwnerFaceArguments }');
 const old='    const checked = checkedOwnerFaceArguments(view, actionId, target, actionArguments);\n    return this.#adapter.submitThermostatInteraction({';
 expect(sdk).toContain(old);
 sdk=sdk.replace(old,'    const current = this.#adapter.thermostatFace().view;\n    const checked = checkedAcknowledgedOwnerFaceArguments(current, view, actionId, target, actionArguments);\n    return this.#adapter.submitThermostatInteraction({');
 const helper=await readFile(new URL('../../targets/browser/sdk/browser-sdk-face-actions.mjs', import.meta.url),'utf8');
 const substitutions=new Map([['sdk/browser-sdk.mjs',sdk],['sdk/browser-sdk-face-actions.mjs',helper]]);
 let originalDigest,fixtureDigest;
 await page.route('**/handbook/application.application.json',async route=>{
  const response=await route.fetch(),manifest=await response.json();originalDigest=manifest.package_digest;
  for(const [path,bytes] of substitutions){const resource=manifest.resources.find(r=>r.path===path);expect(resource).toBeTruthy();resource.sha256=hash(bytes);resource.maximum_bytes=Math.max(resource.maximum_bytes,Buffer.byteLength(bytes));}
  const lines=['conduit.browser/application-package-content@1','application\0'+manifest.application_id,'state\0'+manifest.state_compatibility.identity+'\0'+manifest.state_compatibility.version,...manifest.host_implementations.map(v=>'host-implementation\0'+v)];
  for(const r of manifest.resources)lines.push('resource\0'+[r.role,r.kind,r.path,r.maximum_bytes,r.sha256,r.dependencies.map(d=>d.role+'='+d.specifier).join(',')].join('\0'));
  fixtureDigest=manifest.package_digest=hash(lines.join('\n')+'\n');await route.fulfill({response,json:manifest});
 });
 for(const [path,body]of substitutions)await page.route('**/handbook/'+path,route=>route.fulfill({contentType:'text/javascript',body}));
 await page.goto(root,{waitUntil:'domcontentloaded',timeout:30000});await page.waitForFunction(()=>!!globalThis.__conduitApplication,null,{timeout:30000});
 await page.getByLabel('Choose an example',{exact:true}).selectOption('thermostat/main');await page.getByRole('button',{name:'Try in my Handbook',exact:true}).click();
 const component=page.locator('.handbook-show [data-resident-plot]:visible conduit-owner-face');await expect(component).toBeVisible({timeout:30000});
 const face=await page.evaluate(()=>globalThis.__conduitApplication.thermostat.face());
 const before=await page.evaluate(()=>globalThis.__conduitApplication.snapshot());
 const rejection=await page.evaluate(async face=>{
  const action=face.view.actions.find(a=>a.identity.startsWith('thermostat.')&&a.arguments.length===0);
  if(!action)throw new Error('missing zero-argument Thermostat action');
  const forged={...face.view,face_id:face.view.face_id+'/forged'};
  try{await globalThis.__conduitApplication.thermostat.interact({view:forged,actionId:action.identity,target:action.target,arguments:[],sequence:999});return{accepted:true};}
  catch(error){return{accepted:false,message:error.message};}
 },face);
 expect(rejection.accepted).toBe(false);expect(rejection.message).toContain('acknowledged owner Show');
 const after=await page.evaluate(()=>globalThis.__conduitApplication.snapshot());expect(hash(JSON.stringify(after.evidence.body))).toBe(hash(JSON.stringify(before.evidence.body)));
 const controls=await component.locator('form').evaluateAll(forms=>forms.filter(form=>!form.querySelector('button')?.disabled).map(form=>({identity:form.dataset.ownerAction,text:form.querySelector('button').textContent})));
 const action=controls.find(control=>face.view.actions.some(a=>a.identity===control.identity&&a.arguments.length===0));expect(action).toBeTruthy();
 await component.locator('form').filter({has:page.locator('button',{hasText:action.text})}).getByRole('button',{name:action.text,exact:true}).click();
 await expect(page.locator('[data-check]')).toContainText('Accepted ',{timeout:30000});expect(errors).toEqual([]);
 const report={observed_at_utc:new Date().toISOString(),url:page.url(),browser:browser.version(),fresh_context:true,original_package_digest:originalDigest,fixture_package_digest:fixtureDigest,
 method:'Public runtime with local rehashed substitutions of the Thermostat SDK method and argument helper; stale API request then native button click.',sdk_sha256:hash(sdk),argument_helper_sha256:hash(helper),stale_refusal:rejection,
 body_evidence_sha256_before:hash(JSON.stringify(before.evidence.body)),body_evidence_sha256_after_refusal:hash(JSON.stringify(after.evidence.body)),native_action:action,accepted_status:await page.locator('[data-check]').textContent(),page_errors:errors,
 limits:['Local modified-package fixture, not proof of a deployed SDK change.','No remote-owner transport or full release-gate proof.']};
 await writeFile(receiptPath,JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify({staleRefused:true,nativeAccepted:true,originalDigest,fixtureDigest}));
}finally{await browser.close();}
