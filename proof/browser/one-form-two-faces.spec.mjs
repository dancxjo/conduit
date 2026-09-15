import { spawn } from "node:child_process";
import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { createInterface } from "node:readline";
import { expect, test } from "@playwright/test";

function startJourneyEntrance() {
  const child=spawn("target/debug/patchbay-html",["--one-form-two-faces"],{stdio:["ignore","pipe","pipe"]});
  const errors=[];
  child.stderr.setEncoding("utf8");
  child.stderr.on("data",chunk=>errors.push(chunk));
  const lines=createInterface({input:child.stdout});
  const url=new Promise((resolve,reject)=>{
    lines.once("line",line=>resolve(line.replace("PATCHBAY_HTML_URL=","")));
    child.once("exit",code=>reject(new Error(`two-faces Patchbay exited ${code}: ${errors.join("")}`)));
  });
  return {child,lines,url};
}

test("one Presentation manifests as retained native and pinned-browser faces",async({browser,page})=>{
  const evidenceRoot=process.env.CONDUIT_TWO_FACES_EVIDENCE_ROOT;
  expect(evidenceRoot).toBeTruthy();
  const native=JSON.parse(await readFile(path.join(evidenceRoot,"native.json"),"utf8"));
  const server=startJourneyEntrance();
  try {
    const url=await server.url;
    await page.goto(url);
    const snapshot=await page.evaluate(async()=>(await fetch("/api/snapshot",{cache:"no-store"})).json());
    expect(snapshot.presentation.identity).toBe(native.presentation_id);
    expect(snapshot.presentation.revision).toBe(native.presentation_revision);
    expect(snapshot.presentation.basis).toEqual(native.presentation_basis);
    await expect(page.getByRole("heading",{name:"Entrance choices"})).toBeVisible();
    await expect(page.locator("#flow-root")).toHaveAttribute("data-presentation-id",native.presentation_id);
    await page.screenshot({
      path:path.join(evidenceRoot,"browser.png"),
      fullPage:true,
      animations:"disabled",
      caret:"hide",
      scale:"css",
    });
    const viewport=page.viewportSize();
    const receipt={
      schema:"conduit.journey/one-form-two-faces-browser@1",
      presentation_id:snapshot.presentation.identity,
      presentation_revision:snapshot.presentation.revision,
      presentation_basis:snapshot.presentation.basis,
      renderer_kind:"browser-dom-svg",
      renderer_implementation:"presentation/renderer-dom-svg@1",
      manifestation_id:snapshot.renderer.manifestation.manifestation_id,
      renderer_plan_id:snapshot.renderer.plan.plan_id,
      renderer_play_id:snapshot.renderer.manifestation.active_play_id,
      lifecycle:snapshot.renderer.manifestation.lifecycle.toLowerCase(),
      browser_engine:"chromium",
      browser_version:browser.version(),
      viewport:`${viewport.width}x${viewport.height}`,
      device_scale_factor:"1",
      locale:"en-US",
      timezone:"UTC",
      pixel_equality_claimed:false,
    };
    await writeFile(path.join(evidenceRoot,"browser.json"),`${JSON.stringify(receipt,null,2)}\n`,{encoding:"utf8",flag:"wx"});
  } finally {
    server.lines.close();
    server.child.kill("SIGTERM");
  }
});
