import { spawn } from "node:child_process";
import { mkdir } from "node:fs/promises";
import path from "node:path";
import { createInterface } from "node:readline";
import { expect, test } from "@playwright/test";
import { persistCaptureDeclaration } from "./capture-declarations.mjs";

async function captureCanonical(page,browser,evidenceRoot,name,snapshot,disposition) {
  await page.evaluate(()=>scrollTo(0,0));
  const viewport=page.viewportSize();
  const relativePath=`${name}.png`;
  await page.screenshot({path:path.join(evidenceRoot,relativePath),fullPage:true,animations:"disabled",caret:"hide",scale:"css"});
  await persistCaptureDeclaration(evidenceRoot,{
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
      viewport:`${viewport.width}x${viewport.height}`,
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
}

async function closeDrawer(page,name) {
  if(await page.locator("body").getAttribute(`data-${name}-open`)==="true") {
    await page.locator(`#toggle-${name}`).click();
  }
}

async function prepareCanvasEvidence(page,{inspector=false,structured=false}={}) {
  for(const name of ["palette","parts","truth"])await closeDrawer(page,name);
  const inspectorOpen=await page.locator("body").getAttribute("data-inspector-open")==="true";
  if(inspectorOpen!==inspector)await page.locator("#toggle-inspector").click();
  const structuredOpen=await page.locator("body").getAttribute("data-structured-open")==="true";
  if(structuredOpen!==structured)await page.locator("#toggle-structured").click();
  if(!structured)await page.locator("#fit-flow").click();
}

async function expectFlowDominant(page,{inspector=false}={}) {
  await expect(page.locator("#flow-root .react-flow")).toBeVisible();
  await expect(page.locator("#flow-root .flow-faceplate").first()).toBeVisible();
  const root=await page.locator("#patchbay-root").boundingBox();
  const flow=await page.locator("#flow-root").boundingBox();
  expect(flow.width).toBeGreaterThan(root.width*(inspector ? .55 : .65));
  expect(flow.height).toBeGreaterThan(root.height*.75);
  for(const name of ["palette","parts","truth"]){
    expect(await page.locator("body").getAttribute(`data-${name}-open`)).not.toBe("true");
  }
  if(inspector)await expect(page.locator("#inspector")).toBeVisible();
}

function rectanglesOverlap(left,right) {
  return left.x<right.x+right.width&&left.x+left.width>right.x&&left.y<right.y+right.height&&left.y+left.height>right.y;
}

async function expectFaceplateTextContained(page) {
  const result=await page.locator("#flow-root .flow-faceplate").evaluateAll(faceplates=>faceplates.map(faceplate=>{
    const bounds=element=>{
      const box=element.getBoundingClientRect();
      return {left:box.left,right:box.right,top:box.top,bottom:box.bottom};
    };
    const inside=(inner,outer)=>inner.left>=outer.left-.5&&inner.right<=outer.right+.5&&inner.top>=outer.top-.5&&inner.bottom<=outer.bottom+.5;
    const regions=[...faceplate.querySelectorAll("header,.faceplate-clue,.faceplate-port")];
    const fields=[...faceplate.querySelectorAll(".faceplate-icon,.faceplate-title,.faceplate-role,.faceplate-clue,.faceplate-port-name,.faceplate-port code")];
    return {
      subject:faceplate.dataset.subject,
      fields:fields.map(field=>{
        const owner=field.matches(".faceplate-clue")?field:field.closest("header,.faceplate-port");
        const style=getComputedStyle(field);
        return {className:field.className,inside:inside(bounds(field),bounds(owner)),overflow:style.overflow,textOverflow:style.textOverflow,whiteSpace:style.whiteSpace,title:field.title,text:field.textContent};
      }),
      regions:regions.map(region=>({inside:inside(bounds(region),bounds(faceplate))})),
    };
  }));
  for(const faceplate of result){
    expect(faceplate.subject).toBeTruthy();
    for(const field of faceplate.fields){
      expect(field.inside,`${faceplate.subject} ${field.className} escaped its owned region`).toBe(true);
      expect({overflow:field.overflow,textOverflow:field.textOverflow,whiteSpace:field.whiteSpace}).toEqual({overflow:"hidden",textOverflow:"ellipsis",whiteSpace:"nowrap"});
      expect(field.title).toBeTruthy();
      expect(field.text).toBeTruthy();
    }
    for(const region of faceplate.regions){
      expect(region.inside,`${faceplate.subject} region escaped its faceplate`).toBe(true);
    }
  }
}

async function cordGeometry(page,snapshot) {
  return page.locator("#flow-root .react-flow__edge.flow-cord").evaluateAll((edges,presentation)=>{
    const value=property=>property?.value?.Identity??property?.value?.Text;
    const semanticSubjects=new Map(presentation.properties.filter(property=>property.name==="semantic-id").map(property=>[value(property),property.subject]));
    const cordPorts=presentation.subjects.filter(subject=>subject.role==="Cord").map(cord=>{
      const properties=presentation.properties.filter(property=>property.subject===cord.identity);
      return {id:cord.identity,ports:[semanticSubjects.get(value(properties.find(property=>property.name==="source-port"))),semanticSubjects.get(value(properties.find(property=>property.name==="sink-port")))]};
    }).filter(cord=>cord.ports.every(Boolean)).sort((left,right)=>left.id.localeCompare(right.id));
    const center=element=>{const box=element.getBoundingClientRect();return {x:box.x+box.width/2,y:box.y+box.height/2,node:element.closest(".react-flow__node")?.dataset.id};};
    const screenPoint=(path,offset)=>{const point=path.getPointAtLength(offset),matrix=path.getScreenCTM(),screen=new DOMPoint(point.x,point.y).matrixTransform(matrix);return {x:screen.x,y:screen.y};};
    return edges.map((edge,index)=>{
      const {id,ports}=cordPorts[index],path=edge.querySelector(".react-flow__edge-path"),source=center(document.querySelector(`.faceplate-handle[data-port-id="${CSS.escape(ports[0])}"]`)),target=center(document.querySelector(`.faceplate-handle[data-port-id="${CSS.escape(ports[1])}"]`));
      const length=path.getTotalLength(),start=screenPoint(path,0),end=screenPoint(path,length),direct=Math.hypot(target.x-source.x,target.y-source.y);
      return {id,d:path.getAttribute("d"),marker:path.getAttribute("marker-end"),source,target,start,end,length,direct,forward:target.x>source.x};
    });
  },snapshot.presentation);
}

function startServer() {
  const process = spawn("target/debug/patchbay-html", ["--documentary-fixture"], { stdio:["ignore","pipe","pipe"] });
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
    await expect(page.getByRole("navigation",{name:"Primary"}).getByRole("link")).toHaveCount(3);
    await expect(page.getByRole("link",{name:"Conduit home"})).toHaveAttribute("href","/conduit");
    await expect(page.locator('link[href="/assets/flow.css"]')).toHaveCount(1);
    await expect(page.locator("#flow-root .flow-faceplate").first()).toHaveCSS("width", "240px");
    await expect(page.locator("#status")).toContainText("Presentation revision 1");
    await expect(page.locator("#status")).toContainText("Manifestation Available");
    await expect(page.locator("#status")).toHaveAttribute("data-application-revision", /^\d+$/);
    expect(snapshot.temporal_context).toHaveLength(1);
    expect(snapshot.temporal_context[0].subject).toMatch(/^part\//);
    expect(snapshot.temporal_context[0].relative_time).toMatch(/seconds ago$/);
    await expect(page.locator("#temporal-context")).toContainText(snapshot.temporal_context[0].relative_time);
    await expect(page.locator("#temporal-context")).toContainText("observation");
    await expect(page.locator("#temporal-context details")).not.toHaveAttribute("open", "");
    await page.locator("#temporal-context summary").click();
    await expect(page.locator("#temporal-context details")).toHaveAttribute("open", "");
    await expect(page.locator("#temporal-context details")).toContainText(snapshot.temporal_context[0].source.clock_basis);
    await expect(page.locator("#temporal-context details")).toContainText(snapshot.temporal_context[0].reference.identity);
    await expect(page.locator("#flow-root")).toHaveAttribute("data-renderer","react-flow");
    await expect(page.locator("#flow-root .react-flow")).toBeVisible();
    expect(await page.evaluate(()=>({innerHeight,innerWidth,scrollHeight:document.documentElement.scrollHeight,scrollWidth:document.documentElement.scrollWidth}))).toEqual({innerHeight:768,innerWidth:1366,scrollHeight:768,scrollWidth:1366});
    await page.locator("#toggle-parts").click();
    await expect(page.getByRole("heading",{name:/Parts/})).toBeVisible();
    const truthExplanation=page.locator("#parts-truth-explanation");
    await expect(truthExplanation).toContainText("AVAILABLE means this admitted Part has fresh current Host/Boot presence");
    await expect(truthExplanation).toContainText("LINE READY means this exact Line is currently usable");
    await expect(truthExplanation).toContainText("LINE UNAVAILABLE means this exact Line cannot carry traffic");
    await expect(truthExplanation).toContainText("IN PLAN means the immutable Plan selected this exact Part/Host/Boot realization");
    await expect(truthExplanation).toContainText("PLAYING means an active Play bound to the current Plan includes this Part");
    await expect(page.getByRole("list",{name:"Body Parts"}).getByRole("listitem")).toHaveCount(3);
    await expect(page.getByRole("list",{name:"Body Parts"})).toContainText("HERE · AVAILABLE");
    await expect(page.getByRole("list",{name:"Body Parts"})).toContainText("ATTACHED · AVAILABLE");
    await expect(page.getByRole("list",{name:"Body Parts"})).toContainText("OFFLINE · OFFLINE");
    await expect(page.getByRole("list",{name:"Body Parts"})).toContainText("IN PLAN");
    await expect(page.getByRole("list",{name:"Body Parts"})).toContainText("PLAYING");
    await expect(page.getByRole("list",{name:"Admission candidates"}).getByRole("listitem")).toHaveCount(1);
    await expect(page.getByRole("list",{name:"Admission candidates"})).toContainText("Browser · tab 3");
    await expect(page.locator("#parts-possibilities")).toContainText("current Plan remains unchanged");
    const partPlan=snapshot.presentation.basis.plan_id;
    const candidateInspect=page.getByRole("list",{name:"Admission candidates"}).getByRole("button",{name:"Inspect"});
    await candidateInspect.focus();await candidateInspect.press("Enter");
    await expect(page.locator("#parts-feedback")).toContainText("without admitting it");
    await expect(page.locator("#parts-details")).toContainText(snapshot.parts.wants_to_join[0].candidate_id);
    const inspectedParts=await (await fetch(`${url}/api/snapshot`)).json();
    expect(inspectedParts.parts.wants_to_join).toHaveLength(1);
    expect(inspectedParts.presentation.basis.plan_id).toBe(partPlan);
    await page.getByRole("list",{name:"Admission candidates"}).getByRole("button",{name:"Admit"}).click();
    await expect(page.locator("#parts-feedback")).toContainText("refused nonfatally");
    const refusedParts=await (await fetch(`${url}/api/snapshot`)).json();
    expect(refusedParts.parts.wants_to_join).toHaveLength(1);
    expect(refusedParts.presentation.basis.plan_id).toBe(partPlan);
    await expect(page.getByRole("button",{name:"+ Browser Part"})).toBeVisible();
    await expect(page.getByRole("button",{name:"Plan again"})).toBeVisible();
    await expect(page.getByRole("heading",{name:"Program structure"})).toBeVisible();
    expect(snapshot.entrance.layer).toBe("World");
    expect(snapshot.entrance.selected_subject).toMatch(/^part\//);
    await page.locator("#toggle-parts").click();
    await page.locator("#toggle-truth").click();
    await expect(page.getByRole("heading",{name:"Exact Plan"})).toBeVisible();
    await expect(page.getByRole("heading",{name:"Active Play and Signs"})).toBeVisible();
    await expect(page.locator("#route-cards h3").first()).toContainText("Route");
    await expect(page.locator("#diagnostics li")).toHaveCount(0);
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
    await expect(page.locator("#realizations")).toContainText("base conduit.base/websocket-rfc6455@1");
    await expect(page.locator("#realizations")).toContainText("binding patchbay-renderer/binding/websocket");
    await expect(page.locator("#realizations")).toContainText("base-instance patchbay-renderer/websocket-instance");
    await expect(page.locator("#sign")).toContainText("Renderer patchbay-html/cross-host-prepared · Prepared");
    await expect(page.locator("#sign")).toContainText("Renderer patchbay-html/document-ready · Available");
    await expect(page.locator("#topology li").first()).toContainText("boot");
    await expect(page.locator("#route-cards li").filter({hasText:"conduit.base/usb-cdc-acm@1"}).first()).toBeVisible();
    await expect(page.locator("#route-cards li").filter({hasText:"conduit.base/websocket-rfc6455@1"}).first()).toBeVisible();
    const workspace=await page.locator(".workspace").boundingBox();
    const canvas=await page.locator("#form").boundingBox();
    await page.locator("#toggle-inspector").click();
    const inspector=await page.locator("#inspector").boundingBox();
    expect(workspace).not.toBeNull();expect(canvas).not.toBeNull();expect(inspector).not.toBeNull();
    expect(canvas.width).toBeGreaterThan(workspace.width/2);
    expect(canvas.height).toBeGreaterThan(600);
    expect(canvas.y).toBeLessThan(180);
    expect(inspector.x).toBeGreaterThan(canvas.x);
    await expect(page.getByRole("navigation",{name:"Patchbay workspace"})).toContainText("Linear");

    expect(await page.evaluate(()=>getComputedStyle(document.documentElement).getPropertyValue("--patchbay-theme-identity").trim())).toBe('"conduit.presentation/phosphor@1"');
    expect(await page.evaluate(()=>getComputedStyle(document.body).backgroundColor)).toBe("rgb(5, 7, 11)");
    expect(await page.locator("h1").evaluate(element=>getComputedStyle(element).color)).toBe("rgb(233, 163, 37)");
    await page.evaluate(()=>document.fonts.ready);
    if(canonical) {
      expect(await page.evaluate(()=>document.fonts.check('16px "DejaVu Sans"'))).toBe(true);
      expect((await page.evaluate(()=>getComputedStyle(document.documentElement).fontFamily)).replaceAll('"',"")).toBe("DejaVu Sans, sans-serif");
    }

    await prepareCanvasEvidence(page);
    await expectFlowDominant(page);
    const structuredSummary=await page.locator("#toggle-structured").boundingBox();
    const realizationActions=await page.locator("#front-door-actions").boundingBox();
    const currentStatus=await page.locator("#current-status").boundingBox();
    expect(rectanglesOverlap(structuredSummary,realizationActions)).toBe(false);
    expect(rectanglesOverlap(currentStatus,realizationActions)).toBe(false);
    if(canonical)await captureCanonical(page,browser,evidenceRoot,"overview",snapshot,"full-window-flow-after-semantic-assertions");

    const expectedSubjects=await page.evaluate(async()=>
      (await import("/assets/portable-navigation.js")).projectCurrent(
        await (await fetch("/api/snapshot")).json(),
      ).subjects.map(item=>item.identity).sort(),
    );
    await page.locator("#toggle-palette").click();
    const listSubjects=await page.locator("#subjects [data-subject]").evaluateAll(items=>items.map(item=>item.dataset.subject).sort());
    await page.locator("#toggle-palette").click();
    const principalNode=await page.locator("#flow-root .react-flow__node").first().elementHandle();
    await page.locator("#toggle-structured").click();
    const structuredSubjects=await page.locator("#structured-navigator [data-subject]").evaluateAll(items=>items.map(item=>item.dataset.subject).sort());
    expect(listSubjects).toEqual(expectedSubjects); expect(structuredSubjects).toEqual(expectedSubjects);
    await expect(page.locator("#flow-root .react-flow")).toHaveCount(1);
    await expect(page.locator("#form svg[role=img]")).toHaveCount(0);
    await page.locator("#toggle-structured").click();
    expect(await principalNode.evaluate((node,current)=>node.isSameNode(current),await page.locator("#flow-root .react-flow__node").first().elementHandle())).toBe(true);
    await page.locator("#toggle-palette").click();
    await expect(page.locator("#flow-root .flow-gear")).toHaveCount(snapshot.presentation.subjects.filter(item=>item.role==="Gear").length);
    await expect(page.locator("#flow-root .faceplate-port")).toHaveCount(snapshot.presentation.subjects.filter(item=>item.role==="Port").length);
    await expect(page.locator("#flow-root .flow-cord")).toHaveCount(snapshot.presentation.properties.filter(item=>item.name==="source-port").length);
    await expect(page.locator("#flow-root .flow-cord .react-flow__edge-text").first()).not.toHaveText("Cord");
    const routes=await cordGeometry(page,snapshot);
    expect(routes.length).toBeGreaterThan(0);
    expect(Math.max(...routes.map(route=>Math.hypot(route.start.x-route.source.x,route.start.y-route.source.y)))).toBeLessThan(8);
    expect(Math.max(...routes.map(route=>Math.hypot(route.end.x-route.target.x,route.end.y-route.target.y)))).toBeLessThan(8);
    expect(Math.max(...routes.filter(route=>route.forward).map(route=>route.length/route.direct))).toBeLessThan(2.1);
    expect(routes.every(route=>route.marker?.startsWith("url("))).toBe(true);
    expect(new Set(routes.map(route=>route.d)).size).toBe(routes.length);
    const distinctSourceHandles=routes.filter((route,index)=>routes.some((other,otherIndex)=>otherIndex!==index&&other.source.node===route.source.node&&(other.source.x!==route.source.x||other.source.y!==route.source.y)));
    expect(distinctSourceHandles.every(route=>routes.filter(other=>other.source.node===route.source.node&&(other.source.x!==route.source.x||other.source.y!==route.source.y)).every(other=>Math.hypot(other.source.x-route.source.x,other.source.y-route.source.y)>4))).toBe(true);
    await page.evaluate(()=>window.patchbayReload());
    await expect.poll(async()=>(await cordGeometry(page,snapshot)).length).toBe(routes.length);
    expect((await cordGeometry(page,snapshot)).map(route=>[route.id,route.d])).toEqual(routes.map(route=>[route.id,route.d]));

    const first=page.locator('#subjects button[data-role="Gear"]').first(); await first.focus(); await first.press("Enter");
    const identity=await first.getAttribute("data-subject");
    await expect(first).toHaveAttribute("aria-pressed","true");
    await expect(page.locator("#inspector .inspector-hint")).toContainText("Gear");
    await expect(page.locator("#inspector .exact-selection dd").first()).toHaveText(identity);
    await expect(page.locator(`#flow-root .flow-faceplate[data-subject="${identity.replaceAll('"','\\"')}"]`)).toHaveClass(/semantic-selected/);
    await expect(page.locator("#interaction-proof")).toContainText("Succeeded");
    await expect(page.locator("#interaction-proof")).toContainText("navigation/");
    const selectedSnapshot=await (await fetch(`${url}/api/snapshot`)).json();
    expect(selectedSnapshot.interaction.last_disposition).toBe("Succeeded");
    expect(selectedSnapshot.navigation.cursor.focus).toBe(identity);
    await prepareCanvasEvidence(page,{inspector:true});
    await expectFlowDominant(page,{inspector:true});
    const selectedFaceplate=page.locator(`#flow-root .flow-faceplate[data-subject="${identity.replaceAll('"','\\"')}"]`);
    await expect(selectedFaceplate).toBeVisible();
    await expect(selectedFaceplate).toHaveClass(/semantic-selected/);
    await expectFaceplateTextContained(page);
    await page.locator("#center-flow").click();
    const headingBox=await page.locator(".canvas-heading").boundingBox();
    await expect.poll(async()=>((await selectedFaceplate.boundingBox())?.y??0)).toBeGreaterThan(headingBox.y+headingBox.height);
    const selectedBox=await selectedFaceplate.boundingBox(),inspectorBox=await page.locator("#inspector").boundingBox();
    expect(selectedBox.x+selectedBox.width).toBeLessThan(inspectorBox.x);
    expect(await selectedFaceplate.evaluate(element=>{
      const style=getComputedStyle(element);
      return {width:parseFloat(style.width),maxWidth:parseFloat(style.maxWidth)};
    })).toEqual({width:240,maxWidth:240});
    if(canonical)await captureCanonical(page,browser,evidenceRoot,"selected-gear",selectedSnapshot,"selection-succeeded-and-inspector-correlated");
    const stableLensIdentity={presentation:selectedSnapshot.presentation.identity,plan:selectedSnapshot.presentation.basis.plan_id,play:selectedSnapshot.presentation.basis.active_play_id};
    await page.getByRole("button",{name:"Plan",exact:true}).click();await expect(page.locator("body")).toHaveAttribute("data-lens","plan");await expect(page.locator("#lens-label")).toHaveText("PROGRAM · PLAN");await expect(page.locator("#flow-root .flow-gear")).toHaveCount(3);await expectFlowDominant(page,{inspector:true});
    await expect(page.locator("#flow-root .flow-cord .react-flow__edge-text").first()).toContainText(/item/);
    expect(await page.locator("#flow-root .flow-cord").evaluateAll(edges=>edges.every(edge=>parseFloat(getComputedStyle(edge.querySelector(".react-flow__edge-path")).strokeWidth)>=2))).toBe(true);
    if(canonical)await captureCanonical(page,browser,evidenceRoot,"plan-lens",selectedSnapshot,"same-graph-plan-realization-overlay");
    await page.getByRole("button",{name:"Play",exact:true}).click();await expect(page.locator("#lens-label")).toHaveText("PROGRAM · PLAY");await expect(page.locator("#flow-root .flow-gear")).toHaveCount(3);await expectFlowDominant(page,{inspector:true});
    await expect(page.locator("#flow-root .flow-cord.animated")).toHaveCount(0);
    await expect(page.locator("#flow-root .flow-cord .react-flow__edge-text").first()).toContainText("Completed");
    await expect(page.locator("#flow-root .flow-cord .react-flow__edge-text").first()).toContainText("pressure unavailable");
    if(canonical)await captureCanonical(page,browser,evidenceRoot,"play-lens",selectedSnapshot,"same-graph-active-play-state-and-pressure-overlay");
    await page.getByRole("button",{name:"Signs",exact:true}).click();await expect(page.locator("#lens-label")).toHaveText("PROGRAM · SIGNS");await expect(page.locator("#flow-root .flow-faceplate")).toHaveCount(0);await expect(page.locator("#flow-root .react-flow")).toBeVisible();
    if(canonical)await captureCanonical(page,browser,evidenceRoot,"signs-lens",selectedSnapshot,"same-graph-selected-subject-causal-evidence");
    await page.getByRole("button",{name:"Structure",exact:true}).click();await expect(page.locator("#flow-root .flow-gear")).toHaveCount(3);
    const afterLenses=await (await fetch(`${url}/api/snapshot`)).json();expect({presentation:afterLenses.presentation.identity,plan:afterLenses.presentation.basis.plan_id,play:afterLenses.presentation.basis.active_play_id}).toEqual(stableLensIdentity);expect(afterLenses.navigation.cursor.focus).toBeNull();expect(afterLenses.interaction.revision).toBeGreaterThan(selectedSnapshot.interaction.revision);
    expect(await page.locator("#flow-root .react-flow__node").first().getAttribute("data-id")).toBe(await principalNode.getAttribute("data-id"));
    await prepareCanvasEvidence(page,{structured:true});
    await page.getByRole("button",{name:"Body",exact:true}).click();
    await page.getByRole("button",{name:"Structure",exact:true}).click();
    const route=page.locator('#structured-navigator [data-role="Route"]').first();await route.focus();await route.press("Enter");await expect(route).toHaveAttribute("aria-pressed","true");await expect(page.locator("#inspector .selected-summary")).toContainText("Route");await expect(page.locator("#inspector .exact-selection")).toContainText("sign-new-plan-unsatisfied");
    const routeSnapshot=await (await fetch(`${url}/api/snapshot`)).json();expect(routeSnapshot.navigation.cursor.focus).toBe(await route.getAttribute("data-subject"));
    await expect(page.locator("#deep-inspection")).toBeHidden();
    if(canonical)await captureCanonical(page,browser,evidenceRoot,"route-recovery",routeSnapshot,"exact-line-loss-new-plan-and-same-plan-recovery-spatially-correlated");
    await page.getByRole("button",{name:"Program",exact:true}).click();
    await page.locator("#toggle-palette").click();
    const second=page.locator('#subjects button[data-role="Port"]').first();await second.click();
    await expect(second).toHaveAttribute("aria-pressed","true");
    await expect(page.locator("#inspector .selected-summary")).toContainText(/receiving|outgoing/);
    await expect(page.locator(`#structured-navigator [data-subject="${(await second.getAttribute("data-subject")).replaceAll('"','\\"')}"]`)).toHaveAttribute("aria-pressed","true");
    await expect(page.locator("#interaction-proof")).toContainText("navigation/");
    await page.locator("#toggle-palette").click();
    const third=page.locator('#subjects button[data-role="Cord"]').filter({hasText:"Cord from"}).first();await third.click();
    await expect(third).toHaveAttribute("aria-pressed","true");
    await expect(page.locator("#inspector .selected-summary")).not.toContainText("source-port");
    await expect(page.locator(`#structured-navigator [data-subject="${(await third.getAttribute("data-subject")).replaceAll('"','\\"')}"]`)).toHaveAttribute("aria-pressed","true");
    await expect(page.locator("#interaction-proof")).toContainText("navigation/");
    await page.locator("#toggle-truth").click();
    await page.locator("#toggle-linear").click();
    await expect(page.locator("#interaction-proof")).toContainText("patchbay/interaction/invoke/0");
    await expect(page.locator("#interaction-proof")).toContainText("Succeeded");
    const interactionSnapshot=await (await fetch(`${url}/api/snapshot`)).json();
    expect(interactionSnapshot.interaction.last_disposition).toBe("Succeeded");
    expect(interactionSnapshot.interaction.interaction_plan_id).toBeTruthy();
    expect(interactionSnapshot.interaction.interaction_play_id).toBeTruthy();
    await prepareCanvasEvidence(page,{inspector:true,structured:true});
    if(canonical)await captureCanonical(page,browser,evidenceRoot,"interaction",interactionSnapshot,"control-invocation-plan-play-succeeded");
    const selectedBeforeStale=await page.locator("#subjects [aria-pressed=true]").getAttribute("data-subject");
    const stale=await page.evaluate(async stateForTest=>await (await fetch("/api/interaction",{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({presentation_id:"presentation/stale",presentation_revision:stateForTest.presentation.revision,kind:"select",subject:"dom-node-should-not-apply"})})).json(), snapshot);
    expect(stale.interaction.last_disposition).toBe("Refused(StalePresentation)");
    expect(stale.navigation.cursor.focus).toBe(selectedBeforeStale);
    const unknown=await page.evaluate(async stateForTest=>await (await fetch("/api/interaction",{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({presentation_id:stateForTest.presentation.identity,presentation_revision:stateForTest.presentation.revision,kind:"select",subject:"subject/unknown"})})).json(), snapshot);
    expect(unknown.interaction.last_disposition).toBe("Refused(UnknownSubject)");
    expect(unknown.navigation.cursor.focus).toBe(selectedBeforeStale);
    const staleAction=await page.evaluate(async stateForTest=>{const action=stateForTest.presentation.actions.find(candidate=>candidate.intent==="conduit.intent/toggle-linear-view@1");return await (await fetch("/api/interaction",{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({presentation_id:stateForTest.presentation.identity,presentation_revision:stateForTest.presentation.revision-1,kind:"invoke",action_id:action.identity})})).json();}, snapshot);
    expect(staleAction.interaction.last_disposition).toBe("Refused(StalePresentation)");
    expect(staleAction.navigation.cursor.focus).toBe(selectedBeforeStale);
    await page.evaluate(()=>window.patchbayReload());
    await expect(page.locator("#subjects [aria-pressed=true]")).toHaveAttribute("data-subject",selectedBeforeStale);
    const projectedTextCount=await page.evaluate(async()=>
      (await import("/assets/portable-navigation.js")).projectCurrent(
        await (await fetch("/api/snapshot")).json(),
      ).text.length,
    );
    await expect(page.locator("#linear li")).toHaveCount(projectedTextCount);

    expect(snapshot.renderer.manifestation.lifecycle).toBe("Available");
    const identitiesBefore={content:snapshot.presentation.identity,plan:snapshot.presentation.basis.plan_id,play:snapshot.presentation.basis.active_play_id,manifestation:snapshot.renderer.manifestation.manifestation_id,subjects:listSubjects};
    await page.locator("#toggle-truth").click();
    await page.locator("#toggle-inspector").click();
    await page.locator("#toggle-palette").click();
    await page.locator("#toggle-structured").click();
    const viewportBefore=await page.evaluate(()=>window.patchbayFlowViewport());
    await page.locator("#zoom-in").click();
    await expect.poll(async()=>(await page.evaluate(()=>window.patchbayFlowViewport())).zoom).toBeGreaterThan(viewportBefore.zoom);
    const viewportAfterZoom=await page.evaluate(()=>window.patchbayFlowViewport());
    await page.locator("#pan-right").click();
    await expect.poll(async()=>(await page.evaluate(()=>window.patchbayFlowViewport())).x).toBeGreaterThan(viewportAfterZoom.x);
    await page.locator("#arrange").click();await page.locator("#theme").click();
    await expect(page.locator("#theme")).toHaveAttribute("aria-pressed","true");
    await expect(page.locator("body")).toHaveClass(/high-contrast/);
    expect(await page.locator("#subjects [data-subject]").evaluateAll(items=>items.map(item=>item.dataset.subject).sort())).toEqual(identitiesBefore.subjects);
    await expect(page.locator("#status")).toContainText(identitiesBefore.content);await expect(page.locator("#plan")).toContainText(identitiesBefore.plan);await expect(page.locator("#plan")).toContainText(identitiesBefore.manifestation);await expect(page.locator("#play")).toContainText(identitiesBefore.play);
    const contrastSnapshot=await (await fetch(`${url}/api/snapshot`)).json();
    expect(contrastSnapshot.presentation.identity).toBe(identitiesBefore.content);
    expect(contrastSnapshot.presentation.basis.plan_id).toBe(identitiesBefore.plan);
    expect(contrastSnapshot.presentation.basis.active_play_id).toBe(identitiesBefore.play);
    await prepareCanvasEvidence(page);
    await expectFlowDominant(page);
    if(canonical)await captureCanonical(page,browser,evidenceRoot,"high-contrast",contrastSnapshot,"presentation-changed-semantic-identities-stable");

    const planBefore=await page.locator("#plan").textContent(); await page.reload();
    await expect(page.locator("#plan")).toContainText(snapshot.presentation.basis.plan_id); expect(await page.locator("#plan").textContent()).toBe(planBefore);
    server.process.kill("SIGTERM"); await new Promise(resolve=>server.process.once("exit",resolve));
    await page.evaluate(()=>window.patchbayReload());
    await expect(page.locator("#status")).toContainText("Renderer disconnected; retained revision 1");
    await expect(page.locator("#plan")).toContainText(snapshot.presentation.basis.plan_id);
    await prepareCanvasEvidence(page);
    await expectFlowDominant(page);
    if(canonical)await captureCanonical(page,browser,evidenceRoot,"disconnected",contrastSnapshot,"delivery-unavailable-revision-and-plan-retained");
  } finally { server.lines.close(); if(server.process.exitCode===null)server.process.kill("SIGTERM"); }
});

test("full-window Flow mechanics remain presentation-only", async ({page}) => {
  const server=startServer();
  try {
    const url=await server.url;
    await page.goto(url);
    await expect(page.locator("#flow-root")).toHaveAttribute("data-renderer","react-flow");
    await expect(page.locator("#flow-root .flow-faceplate").first()).toBeVisible();
    const before=await (await fetch(`${url}/api/snapshot`)).json();
    const identities={
      presentation:before.presentation.identity,
      plan:before.presentation.basis.plan_id,
      play:before.presentation.basis.active_play_id,
      manifestation:before.renderer.manifestation.manifestation_id,
      subjects:before.presentation.subjects.map(subject=>subject.identity).sort(),
    };
    const root=await page.locator("#patchbay-root").boundingBox();
    const flow=await page.locator("#flow-root").boundingBox();
    expect(root).toEqual({x:0,y:0,width:1366,height:768});
    expect(flow).toEqual(root);
    expect(await page.evaluate(()=>document.scrollingElement.scrollHeight)).toBe(768);

    const viewport=page.locator("#flow-root .react-flow__viewport");
    const transformBefore=await viewport.getAttribute("style");
    const pane=page.locator("#flow-root .react-flow__pane");
    const paneBox=await pane.boundingBox();
    await page.mouse.move(paneBox.x+paneBox.width/2,paneBox.y+paneBox.height/2);
    await page.mouse.wheel(0,-500);
    await expect.poll(()=>viewport.getAttribute("style")).not.toBe(transformBefore);
    await page.getByRole("button",{name:"Fit",exact:true}).click();

    const nodes=page.locator("#flow-root .react-flow__node");
    expect(await nodes.count()).toBeGreaterThanOrEqual(2);
    const moved=[];
    for(const [index,offset] of [[0,{x:80,y:35}],[1,{x:-65,y:50}]]){
      const node=nodes.nth(index),before=await node.boundingBox();
      await page.mouse.move(before.x+5,before.y+5);
      await page.mouse.down();
      await page.mouse.move(before.x+5+offset.x,before.y+5+offset.y,{steps:5});
      await page.mouse.up();
      const after=await node.boundingBox();
      expect(Math.abs(after.x-before.x)+Math.abs(after.y-before.y)).toBeGreaterThan(40);
      moved.push(after);
    }
    const viewportAfter=await page.evaluate(()=>window.patchbayFlowViewport());
    const presentationOnly=await (await fetch(`${url}/api/snapshot`)).json();
    expect(presentationOnly.interaction.revision).toBe(before.interaction.revision);
    const clickableIndex=await nodes.evaluateAll(items=>items.findIndex(item=>{const box=item.getBoundingClientRect(),hit=document.elementFromPoint(box.x+box.width/2,box.y+box.height/2);return hit?.closest(".react-flow__node")===item;}));
    expect(clickableIndex).toBeGreaterThanOrEqual(0);
    const firstFace=nodes.nth(clickableIndex).getByRole("button");
    await firstFace.click();
    await expect(page.locator("body")).toHaveAttribute("data-inspector-open","true");
    const pointerSelection=await (await fetch(`${url}/api/snapshot`)).json();
    expect(pointerSelection.interaction.revision).toBe(before.interaction.revision+1);
    expect(pointerSelection.interaction.last_request_id).toContain("navigation/");
    await firstFace.focus();
    await firstFace.press("Enter");
    const keyboardSelection=await (await fetch(`${url}/api/snapshot`)).json();
    expect(keyboardSelection.interaction.revision).toBe(pointerSelection.interaction.revision+1);
    expect(keyboardSelection.interaction.last_request_id).toContain("navigation/");
    expect(keyboardSelection.navigation.cursor.focus).toBe(pointerSelection.navigation.cursor.focus);
    await firstFace.press("Space");
    const spaceSelection=await (await fetch(`${url}/api/snapshot`)).json();
    expect(spaceSelection.interaction.revision).toBe(keyboardSelection.interaction.revision+1);
    expect(spaceSelection.interaction.last_request_id).toContain("navigation/");
    expect(spaceSelection.navigation.cursor.focus).toBe(pointerSelection.navigation.cursor.focus);
    const lensAnchor=await nodes.first().boundingBox();
    await page.getByRole("button",{name:"Structure",exact:true}).click();
    await expect(page.locator("#flow-root .flow-faceplate").first()).toHaveAttribute("data-lens","form");
    const intentAnchor=await nodes.first().boundingBox();
    expect(Math.abs(intentAnchor.x-lensAnchor.x)+Math.abs(intentAnchor.y-lensAnchor.y)).toBeLessThan(3);
    await page.getByRole("button",{name:"Plan",exact:true}).click();
    await expect(page.locator("#flow-root .flow-faceplate").first()).toHaveAttribute("data-lens","plan");

    const after=await (await fetch(`${url}/api/snapshot`)).json();
    expect(after.interaction.revision).toBe(spaceSelection.interaction.revision+2);
    expect(after.navigation.cursor.focus).toBeNull();
    expect({
      presentation:after.presentation.identity,
      plan:after.presentation.basis.plan_id,
      play:after.presentation.basis.active_play_id,
      manifestation:after.renderer.manifestation.manifestation_id,
      subjects:after.presentation.subjects.map(subject=>subject.identity).sort(),
    }).toEqual(identities);
    await page.reload();
    await expect(page.locator("#flow-root")).toHaveAttribute("data-presentation-id",identities.presentation);
    await expect.poll(()=>page.evaluate(()=>window.patchbayFlowViewport())).toEqual(viewportAfter);
    for(const [index,after] of moved.entries()){
      await expect.poll(async()=>{
        const restored=await page.locator("#flow-root .react-flow__node").nth(index).boundingBox();
        return Math.abs(restored.x-after.x)+Math.abs(restored.y-after.y);
      }).toBeLessThan(3);
    }
  } finally { server.lines.close(); if(server.process.exitCode===null)server.process.kill("SIGTERM"); }
});

test("narrow enlarged-content workspace has exclusive drawers and restored focus", async ({page,browser},testInfo) => {
  const evidenceRoot=process.env.CONDUIT_EVIDENCE_ROOT;
  const canonical=testInfo.project.name==="chromium"&&Boolean(evidenceRoot);
  if(canonical)await mkdir(evidenceRoot,{recursive:true});
  const server=startServer();
  try {
    const url=await server.url;
    await page.setViewportSize({width:700,height:900});
    await page.emulateMedia({reducedMotion:"reduce"});
    await page.goto(url);
    await expect(page.locator("#flow-root .flow-faceplate").first()).toBeVisible();
    await expectFaceplateTextContained(page);
    expect(await page.locator("#flow-root .flow-faceplate").first().evaluate(element=>({animation:getComputedStyle(element).animationName,transition:getComputedStyle(element).transitionDuration}))).toEqual({animation:"none",transition:"0s"});
    const exactFaceplateTruth=await page.locator("#flow-root .flow-faceplate").evaluateAll(faceplates=>faceplates.map(faceplate=>({subject:faceplate.dataset.subject,accessibilityName:faceplate.getAttribute("aria-label")})));
    await page.locator("#flow-root .flow-faceplate").evaluateAll(faceplates=>{
      const maximal="A human-readable faceplate label that is deliberately much longer than its compact finite visual region · exact/generated/subject/identity/with/no/short/break/opportunity";
      for(const faceplate of faceplates){
        for(const field of faceplate.querySelectorAll(".faceplate-title,.faceplate-clue,.faceplate-port-name,.faceplate-port code")){
          field.dataset.originalText=field.textContent;
          field.dataset.originalTitle=field.title;
          field.textContent=maximal;
          field.title=maximal;
        }
      }
    });
    await page.evaluate(()=>document.documentElement.style.fontSize="200%");
    await expectFaceplateTextContained(page);
    await page.locator("#flow-root .flow-faceplate").evaluateAll(faceplates=>{
      for(const field of faceplates.flatMap(faceplate=>[...faceplate.querySelectorAll("[data-original-text]")])){
        field.textContent=field.dataset.originalText;
        field.title=field.dataset.originalTitle;
        delete field.dataset.originalText;
        delete field.dataset.originalTitle;
      }
    });
    await expectFaceplateTextContained(page);
    expect(await page.locator("#flow-root .flow-faceplate").evaluateAll(faceplates=>faceplates.map(faceplate=>({subject:faceplate.dataset.subject,accessibilityName:faceplate.getAttribute("aria-label")})))).toEqual(exactFaceplateTruth);
    expect(await page.evaluate(()=>({height:document.scrollingElement.scrollHeight,width:document.scrollingElement.scrollWidth}))).toEqual({height:900,width:700});
    const topbarBox=await page.locator(".topbar").boundingBox(),navBox=await page.getByRole("navigation",{name:"Patchbay workspace"}).boundingBox();
    expect(topbarBox.y+topbarBox.height).toBeLessThanOrEqual(navBox.y);
    for(const control of ["Navigate","Parts","Inspector","Exact truth","Program","Body","Structure","Plan","Play","Signs"]){
      const item=page.getByRole("button",{name:control,exact:true});
      await item.evaluate(element=>element.scrollIntoView({block:"nearest",inline:"nearest"}));
      const box=await item.boundingBox();
      expect(box.x).toBeGreaterThanOrEqual(navBox.x);
      expect(box.x+box.width).toBeLessThanOrEqual(navBox.x+navBox.width);
    }

    const navigate=page.locator("#toggle-palette");
    await navigate.click();
    await expect(page.locator("#palette")).toBeVisible();
    expect(await page.evaluate(()=>document.activeElement.closest("#palette")!==null)).toBe(true);
    const paletteBox=await page.locator("#palette").boundingBox();
    expect(paletteBox.x).toBeGreaterThanOrEqual(0);
    expect(paletteBox.x+paletteBox.width).toBeLessThanOrEqual(700);
    expect(await page.locator("#palette").evaluate(element=>element.scrollWidth<=element.clientWidth)).toBe(true);

    const inspectorLauncher=page.locator("#toggle-inspector");
    await inspectorLauncher.click();
    await expect(page.locator("#palette")).toBeHidden();
    await expect(page.locator("#inspector")).toBeVisible();
    expect(await page.evaluate(()=>document.activeElement.closest("#inspector")!==null)).toBe(true);
    await page.keyboard.press("Escape");
    await expect(page.locator("#inspector")).toBeHidden();
    await expect(inspectorLauncher).toBeFocused();

    await navigate.click();
    const snapshot=await (await fetch(`${url}/api/snapshot`)).json();
    const target=snapshot.presentation.actions[0]?.target;
    if(target){
      const structured=page.locator(`#subjects [data-subject="${target.replaceAll('"','\\"')}"]`);
      await structured.click();
      const actions=snapshot.presentation.actions.filter(action=>action.target===target);
      await expect(page.locator("#semantic-actions button")).toHaveCount(actions.length);
      for(const action of actions){
        const control=page.locator(`[data-semantic-action="${action.identity.replaceAll('"','\\"')}"]`);
        await expect(control).toHaveText(action.label.toUpperCase());
        if(action.availability==="Available")await expect(control).toBeEnabled();else await expect(control).toBeDisabled();
      }
      await expect(page.locator("#inspector .exact-selection")).toContainText(target);
      expect(await page.locator("#inspector").evaluate(element=>({horizontal:element.scrollWidth<=element.clientWidth,vertical:getComputedStyle(element).overflowY}))).toEqual({horizontal:true,vertical:"auto"});
      const selected=await (await fetch(`${url}/api/snapshot`)).json();
      expect(selected.navigation.cursor.focus).toBe(target);
      const spatial=page.locator(`#flow-root .react-flow__node[data-id="${target.replaceAll('"','\\"')}"]`);
      if(await spatial.count())await expect(spatial).toHaveClass(/selected/);
    }
    await page.keyboard.press("Escape");
    await expect(page.locator("#inspector")).toBeHidden();
    await page.locator("#fit-flow").click();
    const flowBox=await page.locator("#flow-root").boundingBox();
    expect(flowBox.x).toBeGreaterThanOrEqual(0);
    expect(flowBox.x+flowBox.width).toBeLessThanOrEqual(700);
    expect(flowBox.y+flowBox.height).toBeLessThanOrEqual(900);
    await expect(page.locator("#flow-root .flow-faceplate").first()).toBeVisible();
    if(canonical)await captureCanonical(page,browser,evidenceRoot,"responsive",snapshot,"narrow-enlarged-content-accessibility-after-semantic-assertions");
  } finally { server.lines.close(); if(server.process.exitCode===null)server.process.kill("SIGTERM"); }
});

test("Flow scene reconciliation is finite and identity-exact", async ({page}) => {
  const server=startServer();
  try {
    const url=await server.url;
    const snapshot=await (await fetch(`${url}/api/snapshot`)).json();
    await page.goto(url);
    const result=await page.evaluate(async snapshot=>{
      const scene=await import("/assets/flow-scene.js");
      const layout=await import("/assets/flow-layout.js");
      const canonical={...snapshot,navigation:null};
      const projected=scene.projectFlowScene(canonical);
      const first=scene.reconcileFlowScene(projected);
      first.nodes[0].position={x:913,y:417};
      first.nodes[0].selected=true;
      first.viewport={x:31,y:-27,zoom:1.4};
      const duplicate=scene.reconcileFlowScene(scene.projectFlowScene(canonical),first);
      const added=structuredClone(canonical);
      added.presentation.subjects.push({identity:"subject/new",role:"Gear",label:"New",accessibility_name:"New Gear"});
      const withNew=scene.reconcileFlowScene(scene.projectFlowScene(added),duplicate);
      const withNewAgain=scene.reconcileFlowScene(scene.projectFlowScene(added),duplicate);
      const removed=structuredClone(added);
      removed.presentation.subjects=removed.presentation.subjects.filter(subject=>subject.identity!==first.nodes[0].id);
      const withoutRemoved=scene.reconcileFlowScene(scene.projectFlowScene(removed),withNew);
      const encoded=scene.encodeFlowPresentation(withoutRemoved);
      const decoded=scene.decodeFlowPresentation(encoded,scene.projectFlowScene(removed));
      const acyclic=layout.layoutFlowScene(
        [{id:"a"},{id:"b"},{id:"c"}],
        [{source:"a",target:"b"},{source:"b",target:"c"}],
      );
      const cycleNodes=[{id:"z"},{id:"a"},{id:"m"}],cycleEdges=[{source:"z",target:"a"},{source:"a",target:"z"},{source:"z",target:"m"}];
      const cycle=layout.layoutFlowScene(cycleNodes,cycleEdges);
      const cycleAgain=layout.layoutFlowScene([...cycleNodes].reverse(),[...cycleEdges].reverse());
      const tall=layout.layoutFlowScene([
        {id:"long/a",data:{ports:Array.from({length:12},(_item,index)=>({id:`port/${index}`}))}},
        {id:"long/b",data:{ports:[]}},
      ],[]);
      return {
        anchor:duplicate.nodes.find(node=>node.id===first.nodes[0].id).position,
        localFocus:duplicate.nodes.find(node=>node.id===first.nodes[0].id).selected,
        semanticSelection:projected.nodes.find(node=>node.id===first.nodes[0].id).data.semanticSelected,
        viewport:duplicate.viewport,
        newPosition:withNew.nodes.find(node=>node.id==="subject/new").position,
        repeatedNewPosition:withNewAgain.nodes.find(node=>node.id==="subject/new").position,
        removedSurvived:withoutRemoved.nodes.some(node=>node.id===first.nodes[0].id),
        encodedBytes:encoded.length,
        decodedIds:decoded.nodes.map(node=>node.id).sort(),
        currentIds:withoutRemoved.nodes.map(node=>node.id).sort(),
        oversized:scene.decodeFlowPresentation("x".repeat(scene.MAX_FLOW_STATE_BYTES+1),projected),
        portIds:projected.nodes.flatMap(node=>node.data.ports.map(port=>port.id)),
        handles:projected.edges.map(edge=>[edge.sourceHandle,edge.targetHandle]),
        edgeTypes:projected.edges.map(edge=>edge.type),
        cordLine:projected.edges.map(edge=>({cord:edge.data.semanticIdentity,line:edge.data.lineIdentity})),
        acyclic:[acyclic.get("a"),acyclic.get("b"),acyclic.get("c")],
        cycle:[...cycle.entries()],
        cycleAgain:[...cycleAgain.entries()],
        tallGap:tall.get("long/b").y-tall.get("long/a").y,
      };
    },snapshot);
    expect(result.anchor).toEqual({x:913,y:417});
    expect(result.localFocus).toBe(true);
    expect(typeof result.semanticSelection).toBe("boolean");
    expect(result.viewport).toEqual({x:31,y:-27,zoom:1.4});
    expect(result.newPosition).toBeTruthy();
    expect(result.repeatedNewPosition).toEqual(result.newPosition);
    expect(result.removedSurvived).toBe(false);
    expect(result.encodedBytes).toBeLessThanOrEqual(64*1024);
    expect(result.decodedIds).toEqual(result.currentIds);
    expect(result.oversized).toBeNull();
    expect(result.portIds.length).toBeGreaterThan(0);
    expect(result.handles.length).toBeGreaterThan(0);
    expect(result.handles.flat().every(identity=>result.portIds.includes(identity))).toBe(true);
    expect(result.edgeTypes.every(type=>type==="simplebezier")).toBe(true);
    expect(result.cordLine.every(item=>typeof item.cord==="string"&&(item.line===null||typeof item.line==="string"))).toBe(true);
    expect(result.acyclic[0].x).toBeLessThan(result.acyclic[1].x);
    expect(result.acyclic[1].x).toBeLessThan(result.acyclic[2].x);
    expect(result.cycleAgain).toEqual(result.cycle);
    expect(result.tallGap).toBeGreaterThan(500);
  } finally { server.lines.close(); if(server.process.exitCode===null)server.process.kill("SIGTERM"); }
});
