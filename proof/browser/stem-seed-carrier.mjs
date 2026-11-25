import assert from "node:assert/strict";
import { writeFile } from "node:fs/promises";
import { join } from "node:path";
import { chromium } from "@playwright/test";
import { startStaticProduct } from "./static-product-server.mjs";
const output = process.argv[2];
const site = await startStaticProduct("target/workspace-product", "/conduit/workspace/");
let browser;
try {
  browser = await chromium.launch();
  const page = await browser.newPage();
  await page.route(new URL("stem-fixture-seed.zip", site.url).href, route => route.fulfill({path:join(output,"stem-fixture-seed.zip")}));
  await page.goto(site.url);
  await page.waitForFunction(() => Boolean(globalThis.__conduitBrowserApplication));
  const result = await page.evaluate(async () => {
    const { createExistingComputerAdapter } = await import("./creche-existing-computer.mjs");
    const { readBodyBoundZip } = await import("./creche-native-zip.mjs");
    const application = globalThis.__conduitBrowserApplication;
    const bytes = new Uint8Array(await (await fetch("./stem-fixture-seed.zip")).arrayBuffer());
    const { provision } = readBodyBoundZip(bytes);
    const digest = Array.from(new Uint8Array(await crypto.subtle.digest("SHA-256",bytes)), byte => byte.toString(16).padStart(2,"0")).join("");
    const profile = {target:{id:"browser/wasm32/page"},target_id:"browser/wasm32/page",package_id:"browser-wasm@1",browser_carrier:true};
    const adapter = createExistingComputerAdapter({host:application,profile});
    const binding = {prepared:{...provision.spore,invitation_id:provision.invitation_provision.invitation_id,
      invitation_nonce:provision.invitation_provision.nonce},nativeSpore:{bytes,content_digest:"sha256:"+digest}};
    const signal = new AbortController().signal;
    const mode = "install-existing";
    const obtainment = {private:{release:{manifest:provision.release_manifest}}};
    try {
      const boot = await adapter.realize({mode,obtainment,binding,signal});
      const {join} = await adapter.observe({mode,binding,signal});
      return {terminal:boot.terminal,body:binding.prepared.body_id,host:boot.evidence.host_id,boot:boot.evidence.boot_id,
        joinBody:join.body_id,joinHost:join.host_id,joinBoot:join.boot_id,offers:join.advertisement.capabilities.length,
        signatureBytes:join.signature.length,seed:binding.nativeSpore.content_digest,
        admission:{advertisement:join.advertisement,proof:{invitation_id:join.invitation_id,
          body_id:join.body_id,host_id:join.host_id,boot_id:join.boot_id,nonce:join.nonce,signature:join.signature}}};
    } catch (error) {
      return {error:error.message, cause:error.cause?.message, code:error.code, evidence:error.evidence};
    }
  });
  await writeFile(join(output,"stem-browser-carrier.json"),JSON.stringify(result,null,2));
  if (result.error) throw new Error(JSON.stringify(result));
  assert.equal(result.terminal,"BrowserBundleLoaded");
  assert.equal(result.body,result.joinBody);
  assert.equal(result.host,result.joinHost);
  assert.equal(result.boot,result.joinBoot);
  assert.equal(result.signatureBytes,64);
  assert(result.offers>0);
  console.log(JSON.stringify({ ...result, admission: undefined }));
} finally {
  await browser?.close();
  site.child.kill();
}
