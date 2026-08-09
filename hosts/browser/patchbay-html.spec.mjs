import { spawn } from "node:child_process";
import { createInterface } from "node:readline";
import { expect, test } from "@playwright/test";

function startServer() {
  const process = spawn("target/debug/patchbay-html", [], { stdio:["ignore","pipe","pipe"] });
  const errors=[]; process.stderr.setEncoding("utf8"); process.stderr.on("data",chunk=>errors.push(chunk));
  const lines=createInterface({input:process.stdout});
  const url=new Promise((resolve,reject)=>{lines.once("line",line=>resolve(line.replace("PATCHBAY_HTML_URL=","")));process.once("exit",code=>reject(new Error(`Patchbay HTML exited ${code}: ${errors.join("")}`)));});
  return { process, lines, url };
}

test("HTML Patchbay reconstructs one typed state accessibly and survives delivery loss", async ({page}) => {
  const server=startServer();
  try {
    const url=await server.url;
    const snapshot=await (await fetch(`${url}/api/snapshot`)).json();
    await page.goto(url);
    await expect(page.locator("#status")).toContainText("Presentation revision 1");
    await expect(page.locator("#status")).toContainText("Manifestation Available");
    await expect(page.getByRole("heading",{name:"Form"})).toBeVisible();
    await expect(page.getByRole("heading",{name:"Exact Plan"})).toBeVisible();
    await expect(page.getByRole("heading",{name:"Active Play and Clues"})).toBeVisible();
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
    await expect(page.locator("#clue")).toContainText("Renderer patchbay-html/cross-host-prepared · Prepared");
    await expect(page.locator("#clue")).toContainText("Renderer patchbay-html/document-ready · Available");
    await expect(page.locator("#topology li").first()).toContainText("boot");
    await expect(page.locator("#route-cards li").filter({hasText:"USB CDC"}).first()).toBeVisible();
    await expect(page.locator("#route-cards li").filter({hasText:"WebSocket"}).first()).toBeVisible();

    const expectedSubjects=snapshot.presentation.subjects.map(item=>item.identity).sort();
    const listSubjects=await page.locator("#subjects [data-subject]").evaluateAll(items=>items.map(item=>item.dataset.subject).sort());
    const canvasSubjects=await page.locator("#graph [data-subject]").evaluateAll(items=>items.map(item=>item.dataset.subject).sort());
    expect(listSubjects).toEqual(expectedSubjects); expect(canvasSubjects).toEqual(expectedSubjects);

    const first=page.locator("#subjects button").first(); await first.focus(); await first.press("Enter");
    const identity=await first.getAttribute("data-subject");
    await expect(first).toHaveAttribute("aria-pressed","true");
    await expect(page.locator("#inspector dd").first()).toHaveText(identity);
    expect(await page.locator("#graph [data-subject].selected").getAttribute("data-subject")).toBe(identity);
    await expect(page.locator("#linear li")).toHaveCount(snapshot.presentation.text.length);

    expect(snapshot.renderer.manifestation.lifecycle).toBe("Available");
    const identitiesBefore={content:snapshot.presentation.identity,plan:snapshot.presentation.basis.plan_id,play:snapshot.presentation.basis.active_play_id,manifestation:snapshot.renderer.manifestation.manifestation_id,subjects:listSubjects};
    await page.locator("#zoom-in").click();await page.locator("#pan-right").click();await page.locator("#arrange").click();await page.locator("#theme").click();
    await expect(page.locator("#arrange")).toHaveAttribute("aria-pressed","true");await expect(page.locator("#theme")).toHaveAttribute("aria-pressed","true");
    expect(await page.locator("#subjects [data-subject]").evaluateAll(items=>items.map(item=>item.dataset.subject).sort())).toEqual(identitiesBefore.subjects);
    await expect(page.locator("#status")).toContainText(identitiesBefore.content);await expect(page.locator("#plan")).toContainText(identitiesBefore.plan);await expect(page.locator("#plan")).toContainText(identitiesBefore.manifestation);await expect(page.locator("#play")).toContainText(identitiesBefore.play);

    const planBefore=await page.locator("#plan").textContent(); await page.reload();
    await expect(page.locator("#plan")).toContainText(snapshot.presentation.basis.plan_id); expect(await page.locator("#plan").textContent()).toBe(planBefore);
    server.process.kill("SIGTERM"); await new Promise(resolve=>server.process.once("exit",resolve));
    await page.evaluate(()=>window.patchbayReload());
    await expect(page.locator("#status")).toContainText("Renderer disconnected; retained revision 1");
    await expect(page.locator("#plan")).toContainText(snapshot.presentation.basis.plan_id);
  } finally { server.lines.close(); if(server.process.exitCode===null)server.process.kill("SIGTERM"); }
});
