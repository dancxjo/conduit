// Opt-in observation of the public runtime with a locally rehashed source fixture.
import { chromium, expect } from '@playwright/test';
import { writeFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
const receiptPath = process.argv[2];
if (!receiptPath) throw new Error('an explicit receipt output path is required');
const hash=s=>'sha256:'+createHash('sha256').update(s).digest('hex');
const browser=await chromium.launch({headless:true});
try {
 const context=await browser.newContext();const page=await context.newPage(),errors=[];
 page.on('pageerror',e=>errors.push(e.message));
 const url='https://dancxjo.github.io/conduit/handbook/';
 const response=await context.request.get(url+'plots/convert-pitch.conduit');expect(response.ok()).toBe(true);
 const canonical=await response.text(),malformed=canonical.replace('source = 1kHz','source = "1kHz"');expect(malformed).not.toBe(canonical);
 let originalDigest,fixtureDigest;
 await page.route('**/handbook/application.application.json',async route=>{
  const response=await route.fetch(),manifest=await response.json();originalDigest=manifest.package_digest;
  const source=manifest.resources.find(r=>r.path==='plots/convert-pitch.conduit');source.sha256=hash(malformed);
  const lines=['conduit.browser/application-package-content@1','application\0'+manifest.application_id,
   'state\0'+manifest.state_compatibility.identity+'\0'+manifest.state_compatibility.version,
   ...manifest.host_implementations.map(v=>'host-implementation\0'+v)];
  for(const r of manifest.resources)lines.push('resource\0'+[r.role,r.kind,r.path,r.maximum_bytes,r.sha256,r.dependencies.map(d=>d.role+'='+d.specifier).join(',')].join('\0'));
  fixtureDigest=manifest.package_digest=hash(lines.join('\n')+'\n');
  await route.fulfill({response,json:manifest});
 });
 await page.route('**/handbook/plots/convert-pitch.conduit',route=>route.fulfill({contentType:'text/plain',body:malformed}));
 await page.goto(url,{waitUntil:'domcontentloaded',timeout:30000});
 await page.waitForFunction(()=>!!globalThis.__conduitBrowserApplication,null,{timeout:30000});
 const editor=page.getByRole('textbox',{name:'Plot source',exact:true}),select=page.getByLabel('Choose an example',{exact:true});
 await expect(editor).toBeEnabled();await expect(select).toBeEnabled();
 const option=await select.locator('option').filter({hasText:'kilohertz'}).first().getAttribute('value');expect(option).not.toBeNull();await select.selectOption(option);
 await expect(editor).toHaveValue(malformed);
 const before=await page.evaluate(()=>globalThis.__conduitBrowserApplication.storage.readJson('body-continuity'));expect(before).toBeNull();
 await editor.fill(canonical);await page.getByRole('button',{name:'Try in my Handbook',exact:true}).click();
 await expect(page.locator('[data-check]')).toContainText('Correction saved',{timeout:30000});
 const after=await page.evaluate(()=>globalThis.__conduitBrowserApplication.storage.readJson('body-continuity'));expect(after).toBeNull();
 const saved=await page.evaluate(()=>globalThis.__conduitBrowserApplication.storage.readJson('handbook-source-rescue'));
 expect(saved.some(v=>v.name==='convert-pitch-demo'&&v.original===malformed&&v.source===canonical)).toBe(true);
 await page.getByRole('button',{name:'Try again',exact:true}).click();
 await page.waitForFunction(()=>!!globalThis.__conduitApplication,null,{timeout:30000});
 await select.selectOption('convert-pitch-demo');await expect(editor).toHaveValue(canonical);
 await page.getByRole('button',{name:'Try in my Handbook',exact:true}).click();
 await expect(page.locator('.handbook-show [data-resident-plot]:visible')).toContainText('Exactly 1000 Hz',{timeout:30000});
 const truth=await page.evaluate(()=>globalThis.__conduitApplication.snapshot());expect(errors).toEqual([]);
 const report={observed_at_utc:new Date().toISOString(),url,browser:browser.version(),fresh_context:true,
 method:'One Chromium context; native selectOption/fill/click. HTTPS public runtime with locally intercepted, correctly rehashed malformed lesson/package. No public writes.',
 original_package_digest:originalDigest,fixture_package_digest:fixtureDigest,canonical_source_sha256:hash(canonical),malformed_source:malformed,
 editor_enabled_during_refusal:true,selector_enabled_during_refusal:true,continuity_before_correction:before,continuity_after_correction:after,
 saved_correction:saved,explicit_reload_reopened:true,corrected_output:'Exactly 1000 Hz',body_id_after_reload:truth.bodyId,page_errors:errors,
 limits:['Malformed package is a local interception fixture, not a faulty public deployment.','Does not prove unpromoted live renderer changes.','Historical reporting browser source/cache and malformed retained recovery not inspected.','No release-train or accepted-main receipt verified.']};
 await writeFile(receiptPath,JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify({originalDigest,fixtureDigest,savedCorrection:true,continuityUnchanged:true,correctedOutput:report.corrected_output}));
}finally{await browser.close();}
