import { spawn } from "node:child_process";
import { mkdir, rename, writeFile } from "node:fs/promises";
import path from "node:path";
import { createInterface } from "node:readline";
import { expect, test } from "@playwright/test";

const captureDeclarations=[];

async function captureCanonical(page,browser,evidenceRoot,name,snapshot,disposition) {
  await page.evaluate(()=>scrollTo(0,0));
  const relativePath=`${name}.png`;
  await page.screenshot({path:path.join(evidenceRoot,relativePath),fullPage:true,animations:"disabled",caret:"hide",scale:"css"});
  captureDeclarations.push({
    id:`patchbay.${name}`,
    kind:"screenshot",
    path:relativePath,
    media_type:"image/png",
    required:true,
    provenance:{
      scenario_id:`patchbay-html.${name}@1`,
      step_id:"prove.browser-host.patchbay-html-matrix",
      browser_engine:"chromium",
      browser_version:browser.version(),
      viewport:"1440x1000",
      device_scale_factor:"1",
      locale:"en-US",
      timezone:"UTC",
      presentation_id:snapshot.presentation.identity,
      presentation_revision:String(snapshot.presentation.revision),
      plan_id:snapshot.presentation.basis.plan_id,
      active_play_id:snapshot.presentation.basis.active_play_id,
      manifestation_id:snapshot.renderer.manifestation.manifestation_id,
      renderer_id:"patchbay-html/dom-svg@1",
      asserted_semantic_disposition:disposition,
    },
  });
  const document=JSON.stringify({schema:"conduit.capture-declarations/v1",outputs:captureDeclarations},null,2);
  const temporary=path.join(evidenceRoot,"captures.json.tmp");
  await writeFile(temporary,`${document}\n`,{encoding:"utf8"});
  await rename(temporary,path.join(evidenceRoot,"captures.json"));
}

function startServer() {
  const process = spawn("target/debug/patchbay-html", [], { stdio:["ignore","pipe","pipe"] });
  const errors=[]; process.stderr.setEncoding("utf8"); process.stderr.on("data",chunk=>errors.push(chunk));
  const lines=createInterface({input:process.stdout});
  const url=new Promise((resolve,reject)=>{lines.once("line",line=>resolve(line.replace("PATCHBAY_HTML_URL=","")));process.once("exit",code=>reject(new Error(`Patchbay HTML exited ${code}: ${errors.join("")}`)));});
  return { process, lines, url };
}

test("HTML Patchbay reconstructs one typed state accessibly and survives delivery loss", async ({page,browser},testInfo) => {
  const evidenceRoot=process.env.CONDUIT_EVIDENCE_ROOT;
  const canonical=testInfo.project.name==="chromium"&&Boolean(evidenceRoot);
  if(canonical)await mkdir(evidenceRoot,{recursive:true});
  const server=startServer();
  try {
    const url=await server.url;
    const snapshot=await (await fetch(`${url}/api/snapshot`)).json();
    await page.goto(url);
    await expect(page.locator("#status")).toContainText("Presentation revision 1");
    await expect(page.locator("#status")).toContainText("Manifestation Available");
    await expect(page.getByRole("heading",{name:"Semantic canvas"})).toBeVisible();
    await expect(page.getByRole("heading",{name:"Exact Plan"})).toBeVisible();
    await expect(page.getByRole("heading",{name:"Active Play and Signs"})).toBeVisible();
    await expect(page.locator("#route-cards h3").first()).toContainText("Route");
    await expect(page.locator("#diagnostics li")).toHaveCount(1);
    await expect(page.locator("#diagnostics li")).toContainText("CND-FRM-004");
    await expect(page.locator("#realizations li").first()).toBeVisible();
    await expect(page.locator("#plan")).toContainText("presentation/renderer");
    await expect(page.locator("#plan")).toContainText("presentation/renderer-dom-svg@1");
    await expect(page.locator("#plan")).toContainText("patchbay-html/dom-svg@1");
    await expect(page.locator("#realizations")).toContainText("Port presentation · Input · Info presentation/presentation@1 · Value");
    await expect(page.locator("#realizations")).toContainText("Port manifestation · Output · Info presentation/manifestation@1 · Value");
    await expect(page.locator("#realizations")).toContainText("Resource patchbay-html/host/presentation");
    await expect(page.locator("#realizations")).toContainText("Base conduit.host/present@1 · target presentation/base/dom-svg@1");
    await expect(page.locator("#realizations")).toContainText("project · host patchbay-presentation/host · boot patchbay-presentation/boot");
    await expect(page.locator("#realizations")).toContainText("presentation -> presentation · Info presentation/presentation@1");
    await expect(page.locator("#realizations")).toContainText("Line patchbay-renderer/line/websocket");
    await expect(page.locator("#realizations")).toContainText("base WebSocket");
    await expect(page.locator("#realizations")).toContainText("binding patchbay-renderer/binding/websocket");
    await expect(page.locator("#realizations")).toContainText("base-instance patchbay-renderer/websocket-instance");
    await expect(page.locator("#sign")).toContainText("Renderer patchbay-html/cross-host-prepared · Prepared");
    await expect(page.locator("#sign")).toContainText("Renderer patchbay-html/document-ready · Available");
    await expect(page.locator("#topology li").first()).toContainText("boot");
    await expect(page.locator("#route-cards li").filter({hasText:"USB CDC"}).first()).toBeVisible();
    await expect(page.locator("#route-cards li").filter({hasText:"WebSocket"}).first()).toBeVisible();
    const workspace=await page.locator(".workspace").boundingBox();
    const canvas=await page.locator("#form").boundingBox();
    const inspector=await page.locator("#inspector").boundingBox();
    expect(workspace).not.toBeNull();expect(canvas).not.toBeNull();expect(inspector).not.toBeNull();
    expect(canvas.width).toBeGreaterThan(workspace.width/2);
    expect(canvas.height).toBeGreaterThan(600);
    expect(canvas.y).toBeLessThan(180);
    expect(inspector.x).toBeGreaterThan(canvas.x);
    await expect(page.getByRole("navigation",{name:"Patchbay workspace"})).toContainText("Linear");

    expect(await page.evaluate(()=>getComputedStyle(document.documentElement).getPropertyValue("--patchbay-theme-identity").trim())).toBe('"conduit.patchbay/phosphor@1"');
    expect(await page.evaluate(()=>getComputedStyle(document.body).backgroundColor)).toBe("rgb(5, 7, 11)");
    expect(await page.locator("h1").evaluate(element=>getComputedStyle(element).color)).toBe("rgb(233, 163, 37)");
    await page.evaluate(()=>document.fonts.ready);
    if(canonical) {
      expect(await page.evaluate(()=>document.fonts.check('16px "DejaVu Sans"'))).toBe(true);
      expect((await page.evaluate(()=>getComputedStyle(document.documentElement).fontFamily)).replaceAll('"',"")).toBe("DejaVu Sans, sans-serif");
    }

    if(canonical)await captureCanonical(page,browser,evidenceRoot,"overview",snapshot,"available-after-form-plan-play-and-signs-asserted");

    const expectedSubjects=snapshot.presentation.subjects.map(item=>item.identity).sort();
    const semanticSubjects=snapshot.presentation.subjects.filter(item=>item.role==="Gear"||item.role==="Port"||item.role==="Route"||item.role==="Diagnostic"||(item.role==="Cord"&&snapshot.presentation.properties.some(property=>property.subject===item.identity&&(property.name==="source-port"||property.name==="route-status")))).map(item=>item.identity).sort();
    const listSubjects=await page.locator("#subjects [data-subject]").evaluateAll(items=>items.map(item=>item.dataset.subject).sort());
    const canvasSubjects=await page.locator("#graph [data-subject]").evaluateAll(items=>items.map(item=>item.dataset.subject).sort());
    expect(listSubjects).toEqual(expectedSubjects); expect(canvasSubjects).toEqual(semanticSubjects);
    await expect(page.locator("#inspector .exact-selection")).toBeHidden();
    await expect(page.locator("#graph .gear")).toHaveCount(snapshot.presentation.subjects.filter(item=>item.role==="Gear").length);
    await expect(page.locator("#graph .port")).toHaveCount(snapshot.presentation.subjects.filter(item=>item.role==="Port").length);
    await expect(page.locator("#graph .cord")).toHaveCount(snapshot.presentation.properties.filter(item=>item.name==="source-port").length);
    await expect(page.locator("#graph .diagnostic-overlay")).toHaveCount(snapshot.presentation.subjects.filter(item=>item.role==="Diagnostic").length);
    await expect(page.locator("#graph .route-recovery")).toHaveCount(snapshot.presentation.subjects.filter(item=>item.role==="Route").length);
    await expect(page.locator("#graph .route-candidate.status-unavailable")).toHaveCount(2);
    await expect(page.locator("#graph .route-candidate.status-selected")).toHaveCount(1);
    await expect(page.locator("#graph .route-recovery")).toContainText("UNSATISFIED");
    await expect(page.locator("#graph .route-recovery")).toContainText("replacement Plan");
    await expect(page.locator("#graph .route-recovery")).toContainText("LINE LOST");
    await expect(page.locator("#graph .route-recovery")).toContainText("SELECTED");
    await expect(page.locator("#graph .port-label").first()).toContainText(/>|receiving|outgoing/);
    await expect(page.locator("#graph .gear[data-subject]").first()).toHaveAttribute("aria-label",/Gear/);

    const first=page.locator('#subjects button[data-role="Gear"]').first(); await first.focus(); await first.press("Enter");
    const identity=await first.getAttribute("data-subject");
    await expect(first).toHaveAttribute("aria-pressed","true");
    await expect(page.locator("#inspector .inspector-hint")).toContainText("Gear");
    await expect(page.locator("#inspector .exact-selection dd").first()).toHaveText(identity);
    expect(await page.locator("#graph [data-subject].selected").getAttribute("data-subject")).toBe(identity);
    await expect(page.locator("#graph [data-subject].selected .node")).toHaveCSS("stroke","rgb(244, 196, 0)");
    await expect(page.locator("#interaction-proof")).toContainText("Succeeded");
    await expect(page.locator("#interaction-proof")).toContainText("patchbay/interaction/select/0");
    await expect(page.locator("#interaction-proof")).toContainText("Interaction Plan");
    await expect(page.locator("#interaction-proof")).toContainText("Interaction Play");
    const selectedSnapshot=await (await fetch(`${url}/api/snapshot`)).json();
    expect(selectedSnapshot.interaction.last_disposition).toBe("Succeeded");
    expect(selectedSnapshot.interaction.selected_subject).toBe(identity);
    if(canonical)await captureCanonical(page,browser,evidenceRoot,"selected-gear",selectedSnapshot,"selection-succeeded-and-inspector-correlated");
    const stableLensIdentity={presentation:selectedSnapshot.presentation.identity,plan:selectedSnapshot.presentation.basis.plan_id,play:selectedSnapshot.presentation.basis.active_play_id,selection:selectedSnapshot.interaction.selected_subject,interactionRevision:selectedSnapshot.interaction.revision};
    await page.locator('[data-lens="plan"]').click();await expect(page.locator("body")).toHaveAttribute("data-lens","plan");await expect(page.locator("#lens-label")).toHaveText("PLAN LENS");await expect(page.locator("#graph .gear.selected .plan-overlay")).toBeVisible();await expect(page.locator("#inspector .selected-summary")).toContainText("host-id");
    if(canonical)await captureCanonical(page,browser,evidenceRoot,"plan-lens",selectedSnapshot,"same-graph-plan-realization-overlay");
    await page.locator('[data-lens="play"]').click();await expect(page.locator("#graph .gear.selected .play-overlay")).toBeVisible();await expect(page.locator("#inspector .selected-summary")).toContainText("play-state");await expect(page.locator("#inspector .selected-summary")).toContainText("pressure");
    if(canonical)await captureCanonical(page,browser,evidenceRoot,"play-lens",selectedSnapshot,"same-graph-active-play-state-and-pressure-overlay");
    await page.locator('[data-lens="signs"]').click();await expect(page.locator("#graph .gear.selected .signs-overlay")).toBeVisible();await expect(page.locator("#inspector .selected-summary")).toContainText("Evidence");
    if(canonical)await captureCanonical(page,browser,evidenceRoot,"signs-lens",selectedSnapshot,"same-graph-selected-subject-causal-evidence");
    await page.locator('[data-lens="form"]').click();await expect(page.locator("#graph .gear.selected .plan-overlay")).toBeHidden();await expect(page.locator("#inspector .selected-summary")).toContainText("kind-id");
    const afterLenses=await (await fetch(`${url}/api/snapshot`)).json();expect({presentation:afterLenses.presentation.identity,plan:afterLenses.presentation.basis.plan_id,play:afterLenses.presentation.basis.active_play_id,selection:afterLenses.interaction.selected_subject,interactionRevision:afterLenses.interaction.revision}).toEqual(stableLensIdentity);
    await page.locator('[data-lens="signs"]').click();const route=page.locator('#graph .route-recovery').first();await route.click();await expect(route).toHaveClass(/selected/);await expect(page.locator("#inspector .selected-summary")).toContainText("subject-specific causal Sign");await expect(page.locator("#inspector .exact-selection")).toContainText("sign-new-plan-unsatisfied");
    const routeSnapshot=await (await fetch(`${url}/api/snapshot`)).json();expect(routeSnapshot.interaction.selected_subject).toBe(await route.getAttribute("data-subject"));
    if(canonical)await captureCanonical(page,browser,evidenceRoot,"route-recovery",routeSnapshot,"exact-line-loss-new-plan-and-same-plan-recovery-spatially-correlated");
    await page.locator('[data-lens="form"]').click();
    const second=page.locator('#subjects button[data-role="Port"]').first();await second.click();
    await expect(second).toHaveAttribute("aria-pressed","true");
    await expect(page.locator("#inspector .selected-summary")).toContainText(/receiving|outgoing/);
    expect(await page.locator("#graph .port.selected").getAttribute("data-subject")).toBe(await second.getAttribute("data-subject"));
    await expect(page.locator("#interaction-proof")).toContainText("patchbay/interaction/select/2");
    const third=page.locator('#subjects button[data-role="Cord"]').filter({hasText:"Cord from"}).first();await third.click();
    await expect(third).toHaveAttribute("aria-pressed","true");
    await expect(page.locator("#inspector .selected-summary")).toContainText("source-port");
    expect(await page.locator("#graph .cord.selected").getAttribute("data-subject")).toBe(await third.getAttribute("data-subject"));
    await expect(page.locator("#interaction-proof")).toContainText("patchbay/interaction/select/3");
    const diagnostic=page.locator('#subjects button[data-role="Diagnostic"]').first();await diagnostic.click();
    await expect(diagnostic).toHaveAttribute("aria-pressed","true");
    await expect(page.locator("#inspector .inspector-hint")).toContainText("Diagnostic");
    expect(await page.locator("#graph .diagnostic-overlay.selected").getAttribute("data-subject")).toBe(await diagnostic.getAttribute("data-subject"));
    await expect(page.locator("#interaction-proof")).toContainText("patchbay/interaction/select/4");
    await page.locator("#toggle-linear").click();
    await expect(page.locator("#interaction-proof")).toContainText("patchbay/interaction/invoke/5");
    await expect(page.locator("#interaction-proof")).toContainText("Succeeded");
    const interactionSnapshot=await (await fetch(`${url}/api/snapshot`)).json();
    expect(interactionSnapshot.interaction.last_disposition).toBe("Succeeded");
    expect(interactionSnapshot.interaction.interaction_plan_id).toBeTruthy();
    expect(interactionSnapshot.interaction.interaction_play_id).toBeTruthy();
    if(canonical)await captureCanonical(page,browser,evidenceRoot,"interaction",interactionSnapshot,"control-invocation-plan-play-succeeded");
    const selectedBeforeStale=await page.locator("#subjects [aria-pressed=true]").getAttribute("data-subject");
    const stale=await page.evaluate(async()=>await (await fetch("/api/interaction",{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({presentation_id:"presentation/stale",kind:"select",subject:"dom-node-should-not-apply"})})).json());
    expect(stale.interaction.last_disposition).toBe("Refused(StalePresentation)");
    expect(stale.interaction.selected_subject).toBe(selectedBeforeStale);
    const unknown=await page.evaluate(async stateForTest=>await (await fetch("/api/interaction",{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({presentation_id:stateForTest.presentation.identity,kind:"select",subject:"subject/unknown"})})).json(), snapshot);
    expect(unknown.interaction.last_disposition).toBe("Refused(UnknownSubject)");
    expect(unknown.interaction.selected_subject).toBe(selectedBeforeStale);
    const staleAction=await page.evaluate(async stateForTest=>await (await fetch("/api/interaction",{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({presentation_id:stateForTest.presentation.identity,kind:"invoke",action:"toggle-linear-view",target:"expanded/stale"})})).json(), snapshot);
    expect(staleAction.interaction.last_disposition).toBe("Refused(StalePresentation)");
    expect(staleAction.interaction.selected_subject).toBe(selectedBeforeStale);
    await page.evaluate(()=>window.patchbayReload());
    await expect(page.locator("#subjects [aria-pressed=true]")).toHaveAttribute("data-subject",selectedBeforeStale);
    await expect(page.locator("#linear li")).toHaveCount(snapshot.presentation.text.length);

    expect(snapshot.renderer.manifestation.lifecycle).toBe("Available");
    const identitiesBefore={content:snapshot.presentation.identity,plan:snapshot.presentation.basis.plan_id,play:snapshot.presentation.basis.active_play_id,manifestation:snapshot.renderer.manifestation.manifestation_id,subjects:listSubjects};
    await page.locator("#zoom-in").click();await page.locator("#pan-right").click();await page.locator("#arrange").click();await page.locator("#theme").click();
    await expect(page.locator("#arrange")).toHaveAttribute("aria-pressed","true");await expect(page.locator("#theme")).toHaveAttribute("aria-pressed","true");
    await expect(page.locator("body")).toHaveClass(/high-contrast/);
    expect(await page.locator("#subjects [data-subject]").evaluateAll(items=>items.map(item=>item.dataset.subject).sort())).toEqual(identitiesBefore.subjects);
    await expect(page.locator("#status")).toContainText(identitiesBefore.content);await expect(page.locator("#plan")).toContainText(identitiesBefore.plan);await expect(page.locator("#plan")).toContainText(identitiesBefore.manifestation);await expect(page.locator("#play")).toContainText(identitiesBefore.play);
    const contrastSnapshot=await (await fetch(`${url}/api/snapshot`)).json();
    expect(contrastSnapshot.presentation.identity).toBe(identitiesBefore.content);
    expect(contrastSnapshot.presentation.basis.plan_id).toBe(identitiesBefore.plan);
    expect(contrastSnapshot.presentation.basis.active_play_id).toBe(identitiesBefore.play);
    if(canonical)await captureCanonical(page,browser,evidenceRoot,"high-contrast",contrastSnapshot,"presentation-changed-semantic-identities-stable");

    const planBefore=await page.locator("#plan").textContent(); await page.reload();
    await expect(page.locator("#plan")).toContainText(snapshot.presentation.basis.plan_id); expect(await page.locator("#plan").textContent()).toBe(planBefore);
    server.process.kill("SIGTERM"); await new Promise(resolve=>server.process.once("exit",resolve));
    await page.evaluate(()=>window.patchbayReload());
    await expect(page.locator("#status")).toContainText("Renderer disconnected; retained revision 1");
    await expect(page.locator("#plan")).toContainText(snapshot.presentation.basis.plan_id);
    if(canonical)await captureCanonical(page,browser,evidenceRoot,"disconnected",contrastSnapshot,"delivery-unavailable-revision-and-plan-retained");
  } finally { server.lines.close(); if(server.process.exitCode===null)server.process.kill("SIGTERM"); }
});
