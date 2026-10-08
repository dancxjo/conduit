// Native browser-file follow proof; OS picker bypass is explicit.
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright');
import { readFileSync,writeFileSync,mkdirSync } from 'node:fs';
import { join } from 'node:path';
import { createServer } from 'node:http';
import assert from 'node:assert/strict';
const [htmlPath,tracePath,output]=process.argv.slice(2);
if(!htmlPath||!tracePath||!output)throw Error('Usage: node revision_inspector_follow_browser.mjs inspector.html actual.jsonl output-directory');
mkdirSync(output,{recursive:true});
const html=readFileSync(htmlPath);
const records=readFileSync(tracePath,'utf8').split('\n').filter(line=>line.trim()).map(JSON.parse);
assert(records.length>=3);
const server=createServer((req,res)=>{res.writeHead(200,{'Content-Type':'text/html'});res.end(html)});
await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
const browser=await chromium.launch({headless:true});
try {
 const page=await browser.newPage();
 await page.goto(`http://127.0.0.1:${server.address().port}`);
 await page.evaluate(async line=>{
  const root=await navigator.storage.getDirectory();
  window.proofHandle=await root.getFileHandle('actual-event-prefix.jsonl',{create:true});
  window.writeProof=async text=>{const writer=await window.proofHandle.createWritable();await writer.write(text);await writer.close()};
  await window.writeProof(line+'\n');
  // Bypass the OS picker only; retain an actual native FileSystemFileHandle.
  window.showOpenFilePicker=async()=>[window.proofHandle];
 },JSON.stringify(records[0]));
 await page.locator('#event').fill('6');await page.locator('#event').dispatchEvent('input');
 await page.locator('#follow').click();
 await page.waitForFunction(()=>document.getElementById('ordinal').textContent==='1 / 1');
 const first=JSON.stringify(records[0])+'\n', second=JSON.stringify(records[1]);
 await page.evaluate(text=>window.writeProof(text),first+second.slice(0,Math.floor(second.length/2)));
 await page.waitForTimeout(1200);
 assert.equal(await page.locator('#ordinal').innerText(),'1 / 1');
 assert.equal(await page.locator('#error').innerText(),'');
 await page.evaluate(text=>window.writeProof(text),first+second+'\n');
 await page.waitForFunction(()=>document.getElementById('ordinal').textContent==='2 / 2');
 assert.deepEqual(JSON.parse(await page.locator('#raw').textContent()),records[1]);
 // Fault injection: valid JSON cannot rewrite an already observed event.
 const rewritten=JSON.stringify({...records[0],event:'forged-history'})+'\n'+second+'\n';
 await page.evaluate(text=>window.writeProof(text),rewritten);
 await page.waitForFunction(()=>document.getElementById('error').textContent.includes('history changed'));
 assert.equal(await page.locator('#ordinal').innerText(),'2 / 2');
 assert.deepEqual(JSON.parse(await page.locator('#raw').textContent()),records[1]);
 // An unfinished UTF-8 code point in the unflushed line is not a new event.
 await page.evaluate(async text=>{
  const prefix=new TextEncoder().encode(text),bytes=new Uint8Array(prefix.length+3);
  bytes.set(prefix);bytes.set([0x7b,0x22,0xc3],prefix.length);
  await window.writeProof(bytes);
 },first+second+'\n');
 await page.waitForFunction(()=>document.getElementById('error').textContent==='');
 assert.equal(await page.locator('#ordinal').innerText(),'2 / 2');
 await page.evaluate(text=>window.writeProof(text),first+second+'\n{broken}\n');
 await page.waitForFunction(()=>document.getElementById('error').textContent.length>0);
 assert.equal(await page.locator('#ordinal').innerText(),'2 / 2');
 await page.locator('#stop').click();
 await page.evaluate(text=>window.writeProof(text),first+second+'\n'+JSON.stringify(records[2])+'\n');
 await page.waitForTimeout(1200);
 assert.equal(await page.locator('#ordinal').innerText(),'2 / 2');
 assert(await page.locator('#stop').isDisabled());
 const result={native_browser_file_handle:true,flushed_append_observed:true,incomplete_line_ignored:true,incomplete_utf8_line_ignored:true,rewritten_history_preserves_previous:true,malformed_append_preserves_previous:true,stop_preserves_previous:true,os_picker_verified:false,parser_executed:false,events_from_actual_trace:true};
 writeFileSync(join(output,'native-follow-proof.json'),JSON.stringify(result,null,2)+'\n');
 console.log(JSON.stringify(result));
}finally{await browser.close();await new Promise(resolve=>server.close(resolve));}
