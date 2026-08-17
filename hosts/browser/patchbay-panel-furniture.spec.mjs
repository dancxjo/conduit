import { test, expect } from "@playwright/test";
import { spawn } from "node:child_process";
import { createInterface } from "node:readline";

function startServer() {
  const process=spawn("target/debug/patchbay-html",["--documentary-fixture"],{stdio:["ignore","pipe","pipe"]});
  const errors=[];process.stderr.setEncoding("utf8");process.stderr.on("data",chunk=>errors.push(chunk));
  const lines=createInterface({input:process.stdout});
  const url=new Promise((resolve,reject)=>{lines.once("line",line=>resolve(line.replace("PATCHBAY_HTML_URL=","")));process.once("exit",code=>reject(new Error(`Patchbay HTML exited ${code}: ${errors.join("")}`)));});
  return {process,lines,url};
}

function semanticIdentity(snapshot) {
  return {
    presentation:snapshot.presentation.identity,
    revision:snapshot.presentation.revision,
    plan:snapshot.presentation.basis.plan_id,
    play:snapshot.presentation.basis.active_play_id,
    selection:snapshot.interaction.selected_subject,
    cursor:snapshot.navigation.cursor,
    manifestation:snapshot.renderer.manifestation.manifestation_id,
    actions:snapshot.presentation.actions.map(action=>({identity:action.identity,target:action.target,availability:action.availability})),
  };
}

async function enactFurnitureControl(page,name,enact) {
  const navigation=["truth","inspector"].includes(name)?page.waitForResponse(response=>
    response.url().endsWith("/api/navigation")&&response.request().method()==="POST",
  ):null;
  await enact();
  if(navigation){expect((await (await navigation).json()).interaction.last_disposition).toBe("Succeeded");await expect(page.locator("#deep-inspection")).not.toHaveAttribute("aria-busy","true");}
}

test("subordinate furniture is keyboard-operable and presentation-only",async ({page})=>{
  const server=startServer();
  try {
    const url=await server.url;
    let interactionPosts=0;
    page.on("request",request=>{if(request.method()==="POST"&&request.url().endsWith("/api/interaction"))interactionPosts+=1;});
    await page.goto(url);
    const before=semanticIdentity(await (await fetch(`${url}/api/snapshot`)).json());
    const surfaces=[
      ["palette","#palette","#toggle-palette"],
      ["parts","#parts","#toggle-parts"],
      ["inspector","#inspector","#toggle-inspector"],
      ["truth","#deep-inspection","#toggle-truth"],
      ["structured","#structured-navigator","#toggle-structured"],
    ];
    await expect(page.locator("[data-furniture-surface] .furniture-bar")).toHaveCount(surfaces.length);
    for(const [name,selector,launcherSelector] of surfaces) {
      const surface=page.locator(selector),launcher=page.locator(launcherSelector);
      if(name==="inspector") {
        let holdNavigation,releaseNavigation;
        const navigationHeld=new Promise(resolve=>{holdNavigation=resolve;});
        const navigationReleased=new Promise(resolve=>{releaseNavigation=resolve;});
        const delayInspectorNavigation=async route=>{
          if(route.request().method()==="POST") {holdNavigation();await navigationReleased;}
          await route.continue();
        };
        await page.route("**/api/navigation",delayInspectorNavigation);
        await launcher.focus();await launcher.press("Enter");await navigationHeld;
        await expect(surface).toHaveAttribute("aria-busy","true");
        await expect(surface.locator('[data-furniture-action="close"]')).toBeDisabled();
        await page.keyboard.press("Escape");
        releaseNavigation();
        await expect(surface).not.toHaveAttribute("aria-busy","true");
        await page.unroute("**/api/navigation",delayInspectorNavigation);
        await expect(surface).toBeHidden();await expect(launcher).toBeFocused();
      }
      await launcher.focus();await enactFurnitureControl(page,name,()=>launcher.press("Enter"));
      await expect(surface).toBeVisible();
      await expect(surface).toHaveAttribute("data-furniture-surface",name);
      await expect(surface).toHaveAttribute("data-furniture-collapsed","false");
      await expect(surface.locator("[data-furniture-action]")).toHaveCount(3);
      await expect(surface.locator('[data-furniture-action="collapse"]')).toBeFocused();
      const initialDock=await surface.getAttribute("data-furniture-dock");
      await surface.locator('[data-furniture-action="collapse"]').press("Enter");
      await expect(surface).toHaveAttribute("data-furniture-collapsed","true");
      await surface.locator('[data-furniture-action="collapse"]').press("Enter");
      await expect(surface).toHaveAttribute("data-furniture-collapsed","false");
      if(name==="inspector") {
        const constrainedFlow=await page.locator("#flow-root").boundingBox();
        await surface.locator('[data-furniture-action="close"]').focus();
        await enactFurnitureControl(page,name,()=>surface.locator('[data-furniture-action="close"]').press("Enter"));
        await expect(surface).toBeHidden();
        expect((await page.locator("#flow-root").boundingBox()).width).toBeGreaterThan(constrainedFlow.width);
        await enactFurnitureControl(page,name,()=>launcher.press("Enter"));
        await expect(surface).toBeVisible();
        await expect(surface).toHaveAttribute("data-furniture-collapsed","false");
      }
      await surface.locator('[data-furniture-action="move"]').focus();
      await surface.locator('[data-furniture-action="move"]').press("Enter");
      await expect(surface).not.toHaveAttribute("data-furniture-dock",initialDock);
      const movedDock=await surface.getAttribute("data-furniture-dock");
      const moved=await surface.boundingBox(),viewport=page.viewportSize();
      expect(moved.x).toBeGreaterThanOrEqual(0);expect(moved.y).toBeGreaterThanOrEqual(0);
      expect(moved.x+moved.width).toBeLessThanOrEqual(viewport.width);
      expect(moved.y+moved.height).toBeLessThanOrEqual(viewport.height);
      await surface.locator('[data-furniture-action="close"]').focus();
      await enactFurnitureControl(page,name,()=>surface.locator('[data-furniture-action="close"]').press("Enter"));
      await expect(surface).toBeHidden();await expect(launcher).toBeFocused();
      await enactFurnitureControl(page,name,()=>launcher.press("Enter"));
      await expect(surface).toBeVisible();
      await expect(surface).toHaveAttribute("data-furniture-collapsed","false");
      await expect(surface).toHaveAttribute("data-furniture-dock",movedDock);
      await expect(surface.locator('[data-furniture-action="collapse"]')).toBeFocused();
      await enactFurnitureControl(page,name,()=>page.keyboard.press("Escape"));
      await expect(surface).toBeHidden();await expect(launcher).toBeFocused();
    }
    await page.setViewportSize({width:700,height:900});
    const inspectorLauncher=page.locator("#toggle-inspector"),inspector=page.locator("#inspector");
    await inspectorLauncher.focus();await inspectorLauncher.press("Enter");
    const restored=await inspector.boundingBox(),viewport=page.viewportSize();
    expect(restored.x).toBeGreaterThanOrEqual(0);expect(restored.y).toBeGreaterThanOrEqual(0);
    expect(restored.x+restored.width).toBeLessThanOrEqual(viewport.width);
    expect(restored.y+restored.height).toBeLessThanOrEqual(viewport.height);
    await page.keyboard.press("Escape");
    await expect(inspector).toBeHidden();
    expect(interactionPosts).toBe(0);
    await expect.poll(async()=>semanticIdentity(await (await fetch(`${url}/api/snapshot`)).json())).toEqual(before);
  } finally {server.process.kill();server.lines.close();}
});
