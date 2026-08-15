import { mkdir, readFile } from "node:fs/promises";
import path from "node:path";
import { expect, test } from "@playwright/test";
import { persistCaptureDeclaration } from "./capture-declarations.mjs";

const evidenceRoot=process.env.CONDUIT_EVIDENCE_ROOT;

function declaration(browser,testInfo,name) {
  const viewport=testInfo.project.use.viewport;
  return {
    id:`patchbay.${name}`,
    kind:"screenshot",
    path:`${name}.png`,
    media_type:"image/png",
    required:true,
    provenance:{
      scenario_id:`patchbay-html.${name}@1`,
      step_id:"prove.browser-host.patchbay-html-matrix",
      browser_engine:"chromium",
      browser_version:browser.version(),
      viewport:`${viewport.width}x${viewport.height}`,
      device_scale_factor:"1",
      locale:"en-US",
      timezone:"UTC",
      presentation_id:"presentation/capture-restart-probe",
      presentation_revision:"1",
      plan_id:"plan/capture-restart-probe",
      active_play_id:"play/capture-restart-probe",
      manifestation_id:"manifestation/capture-restart-probe",
      renderer_id:"patchbay-html/dom-svg@1",
      asserted_semantic_disposition:`capture-before-induced-failure-worker-${testInfo.workerIndex}`,
    },
  };
}

async function capture(page,browser,testInfo,name) {
  await page.setContent(`<main><h1>${name}</h1><p>Deterministic capture restart diagnostic.</p></main>`);
  await page.screenshot({path:path.join(evidenceRoot,`${name}.png`),fullPage:true,animations:"disabled",caret:"hide",scale:"css"});
  await persistCaptureDeclaration(evidenceRoot,declaration(browser,testInfo,name));
}

test.beforeAll(async()=>{
  expect(evidenceRoot,"CONDUIT_EVIDENCE_ROOT is required").toBeTruthy();
  await mkdir(evidenceRoot,{recursive:true});
});

test("capture declarations survive an induced proof failure",async({page,browser},testInfo)=>{
  await capture(page,browser,testInfo,"overview");
  expect("induced browser proof failure").toBe("successful proof");
});

test("restarted worker merges the retained declaration",async({page,browser},testInfo)=>{
  const before=JSON.parse(await readFile(path.join(evidenceRoot,"captures.json"),"utf8"));
  expect(before.outputs.map(output=>output.id)).toEqual(["patchbay.overview"]);
  expect(before.outputs[0].provenance.asserted_semantic_disposition).not.toContain(`worker-${testInfo.workerIndex}`);
  await capture(page,browser,testInfo,"selected-gear");
  const after=JSON.parse(await readFile(path.join(evidenceRoot,"captures.json"),"utf8"));
  expect(after.outputs.map(output=>output.id)).toEqual(["patchbay.overview","patchbay.selected-gear"]);
});
