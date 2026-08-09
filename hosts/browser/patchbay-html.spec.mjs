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
    await expect(page.locator("#status")).toContainText("Snapshot revision 1");
    await expect(page.getByRole("heading",{name:"Form"})).toBeVisible();
    await expect(page.getByRole("heading",{name:"Exact Plan"})).toBeVisible();
    await expect(page.getByRole("heading",{name:"Active Play and evidence"})).toBeVisible();
    await expect(page.getByRole("heading",{name:"New-Plan recovery"})).toBeVisible();
    await expect(page.getByRole("heading",{name:"Same-Plan fallback"})).toBeVisible();
    await expect(page.getByRole("heading",{name:"Refused ambient route"})).toBeVisible();
    await expect(page.locator("#diagnostics li")).toHaveCount(1);
    await expect(page.locator("#diagnostics li")).toContainText("bytes");
    await expect(page.locator("#realizations li").first()).toContainText("implementation");
    await expect(page.locator("#topology li").first()).toContainText("boot");
    await expect(page.locator("#route-cards li").filter({hasText:"USB CDC"}).first()).toBeVisible();
    await expect(page.locator("#route-cards li").filter({hasText:"WebSocket"}).first()).toBeVisible();

    const expectedSubjects=snapshot.document.forms.find(form=>form.name===snapshot.document.open_form).items.map(item=>item.identity).sort();
    const listSubjects=await page.locator("#subjects [data-subject]").evaluateAll(items=>items.map(item=>item.dataset.subject).sort());
    const canvasSubjects=await page.locator("#graph [data-subject]").evaluateAll(items=>items.map(item=>item.dataset.subject).filter(identity=>!identity.includes("/stage/")).sort());
    expect(listSubjects).toEqual(expectedSubjects); expect(canvasSubjects).toEqual(expectedSubjects);

    const first=page.locator("#subjects button").first(); await first.focus(); await first.press("Enter");
    const identity=await first.getAttribute("data-subject");
    await expect(first).toHaveAttribute("aria-pressed","true");
    await expect(page.locator("#inspector dd").first()).toHaveText(identity);
    expect(await page.locator("#graph [data-subject].selected").getAttribute("data-subject")).toBe(identity);
    await expect(page.locator("#linear li")).toHaveCount(snapshot.linear.length);

    const identitiesBefore={plan:snapshot.plan.plan_id,play:snapshot.play.active_play_id,subjects:listSubjects};
    await page.locator("#zoom-in").click();await page.locator("#pan-right").click();await page.locator("#arrange").click();await page.locator("#theme").click();
    await expect(page.locator("#arrange")).toHaveAttribute("aria-pressed","true");await expect(page.locator("#theme")).toHaveAttribute("aria-pressed","true");
    expect(await page.locator("#subjects [data-subject]").evaluateAll(items=>items.map(item=>item.dataset.subject).sort())).toEqual(identitiesBefore.subjects);
    await expect(page.locator("#plan")).toContainText(identitiesBefore.plan);await expect(page.locator("#play")).toContainText(identitiesBefore.play);

    const planBefore=await page.locator("#plan").textContent(); await page.reload();
    await expect(page.locator("#plan")).toContainText(snapshot.plan.plan_id); expect(await page.locator("#plan").textContent()).toBe(planBefore);
    server.process.kill("SIGTERM"); await new Promise(resolve=>server.process.once("exit",resolve));
    await page.evaluate(()=>window.patchbayReload());
    await expect(page.locator("#status")).toContainText("Renderer disconnected; retained revision 1");
    await expect(page.locator("#plan")).toContainText(snapshot.plan.plan_id);
  } finally { server.lines.close(); if(server.process.exitCode===null)server.process.kill("SIGTERM"); }
});
