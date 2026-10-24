import {createRequire} from 'node:module';
import {spawn,execFileSync} from 'node:child_process';
import {createInterface} from 'node:readline';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import assert from 'node:assert/strict';
const repo=process.cwd(),require=createRequire(repo+'/proof/browser/package.json'),{chromium}=require('@playwright/test');
const source=execFileSync('git',['rev-parse','HEAD'],{encoding:'utf8'}).trim();
assert.equal(execFileSync('git',['status','--porcelain'],{encoding:'utf8'}).trim(),'');
const output=repo+'/../tmp/browser-narrow-detail-final';await mkdir(output);
const binary=repo+'/target/debug/conduit-browser-patchbay-workbench';
const child=spawn(binary,['--documentary-fixture'],{stdio:['ignore','pipe','pipe']});
let stderr='';child.stderr.on('data',chunk=>stderr+=String(chunk));
const lines=createInterface({input:child.stdout});
const url=await new Promise((resolve,reject)=>{lines.once('line',line=>resolve(line.replace('PATCHBAY_HTML_URL=','')));child.once('exit',code=>reject(new Error(`server ${code}: ${stderr}`)));});
const browser=await chromium.launch({headless:true});
try{
 const context=await browser.newContext({viewport:{width:390,height:844},locale:'en-US',timezoneId:'UTC',colorScheme:'dark',reducedMotion:'reduce'}),page=await context.newPage();
 await page.goto(url);await page.locator('body[data-application-ready="true"]').waitFor();
 await page.getByRole('button',{name:'Plot',exact:true}).click();await page.locator('.flow-frontplate').first().waitFor();
 const baseline=await (await page.request.get(url+'/api/snapshot')).json();
 await page.getByRole('combobox',{name:'Look',exact:true}).selectOption('conduit.look/navy@1');
 await page.addStyleTag({content:'html{font-size:32px!important}'});
 await page.getByRole('button',{name:'Arrange',exact:true}).click();
 await page.waitForFunction(()=>{
  const rects=[...document.querySelectorAll('.flow-frontplate')].map(node=>node.getBoundingClientRect());
  return rects.length>0&&rects.every((a,i)=>rects.slice(i+1).every(b=>Math.min(a.right,b.right)-Math.max(a.left,b.left)<=1||Math.min(a.bottom,b.bottom)-Math.max(a.top,b.top)<=1));
 });
 await page.locator('#flow-root').scrollIntoViewIfNeeded();await page.screenshot({path:output+'/narrow-graph-navy-overview.png',animations:'disabled'});
 for(let step=0;step<7;step++)await page.getByRole('button',{name:'Zoom in',exact:true}).click();
 await page.locator('#flow-root').scrollIntoViewIfNeeded();await page.screenshot({path:output+'/narrow-graph-navy-detail.png',animations:'disabled'});
 const current=await (await page.request.get(url+'/api/snapshot')).json();assert.deepEqual(current.face,baseline.face);
 const observation=await page.evaluate(()=>({font:getComputedStyle(document.documentElement).fontSize,look:document.body.dataset.applicationLook,viewport:window.patchbayFlowViewport(),documentHeight:document.documentElement.scrollHeight,graphHeight:document.getElementById('flow-root').getBoundingClientRect().height}));
 assert.equal(observation.font,'32px');assert.equal(observation.look,'conduit.look/navy@1');assert.ok(observation.graphHeight>=320);
 await writeFile(output+'/receipt.json',JSON.stringify({source,binary_sha256:createHash('sha256').update(await readFile(binary)).digest('hex'),proof_class:'documentary-browser-capture',browser:browser.version(),viewport:{width:390,height:844},zoom_in_actions:7,unchanged_face:true,observation,source_unchanged:execFileSync('git',['rev-parse','HEAD'],{encoding:'utf8'}).trim()===source,tracked_tree_clean:execFileSync('git',['status','--porcelain'],{encoding:'utf8'}).trim()===''},null,2));
 console.log(JSON.stringify(observation));
}finally{await browser.close();lines.close();child.kill('SIGTERM');}
