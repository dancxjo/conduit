import { spawn } from "node:child_process";
import { createInterface } from "node:readline";
import { writeFile, readFile, mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { expect, test } from "@playwright/test";

async function entrance(args = []) {
  const child = spawn("target/debug/conduit-browser-patchbay-workbench", args, {stdio:["ignore","pipe","pipe"]});
  let stderr = ""; child.stderr.on("data", chunk => { stderr += chunk; });
  const lines = createInterface({input:child.stdout});
  const url = await new Promise((resolve,reject) => {
    lines.once("line", line => resolve(line.replace("PATCHBAY_HTML_URL=", "")));
    child.once("error", reject);
    child.once("exit", code => reject(new Error(`Patchbay exited ${code}: ${stderr}`)));
  });
  return {url, async close() { child.kill("SIGTERM"); if (child.exitCode === null) await new Promise(resolve => child.once("exit",resolve)); lines.close(); }};
}
const snapshot = page => page.evaluate(async () => (await fetch("/api/snapshot", {cache:"no-store"})).json());
function fact(face, subject, name) {
  const value = face.properties.find(item => item.subject === subject && item.name === name)?.value;
  return value?.Identity ?? value?.Text ?? value?.Count ?? value?.Flag;
}

for (const width of [1440, 980]) test(`ordinary Morse encounter and checked refusal at ${width}px`, async ({page,browser}, info) => {
  test.setTimeout(90_000);
  await page.setViewportSize({width,height:1000});
  const server = await entrance(), errors = [], network = [];
  page.on("pageerror", error => errors.push(error.message));
  page.on("request", request => { if (request.url().includes("/api/")) network.push({event:"request",url:request.url().split("/").at(-1),method:request.method(),operation:request.method()==="POST"?request.postDataJSON()?.operation:undefined,time:Date.now()}); });
  page.on("response", response => { if (response.url().includes("/api/")) network.push({event:"response",url:response.url().split("/").at(-1),status:response.status(),time:Date.now()}); });
  try {
    await page.goto(server.url);
    const open = page.getByRole("button", {name:"Open Plot Morse Network",exact:true});
    await expect(open).toBeVisible();
    await expect(page.getByRole("heading", {name:"Make the system you mean"})).toBeVisible();
    const initial = await snapshot(page);
    expect(initial.face.basis.body_id).toBeNull();
    await page.screenshot({path:info.outputPath("entrance.png")});
    await open.click();
    await expect(page.locator(".flow-frontplate.role-gear")).toHaveCount(4);
    const opened = await snapshot(page);
    expect(opened.face.basis.body_id).toBeNull();
    expect(opened.face.basis.plan_id).toBeNull();
    expect(opened.face.basis.active_play_id).toBeNull();
    expect(opened.authoring.palette.length).toBeGreaterThan(80); // the former five-slot rendering bound
    await expect(page.locator("#gear-results button")).toHaveCount(opened.authoring.palette.length);
    await expect(page.locator(".react-flow__edge.flow-cord")).toHaveCount(3);
    await expect(page.locator(".faceplate-port")).toHaveCount(7);
    expect((await page.locator(".flow-frontplate.role-gear").evaluateAll(nodes => nodes.map(node=>node.dataset.subject).sort())))
      .toEqual(opened.face.subjects.filter(subject=>subject.role==="Gear").map(subject=>subject.identity).sort());
    await page.getByRole("button",{name:"Fit",exact:true}).click();
    await page.screenshot({path:info.outputPath("morse-graph.png")});
    const zoom = await page.evaluate(() => window.patchbayFlowViewport().zoom);
    await page.getByRole("button", {name:"Zoom in",exact:true}).click();
    await expect.poll(() => page.evaluate(() => window.patchbayFlowViewport().zoom)).toBeGreaterThan(zoom);
    expect((await snapshot(page)).authoring.source_document_id).toBe(opened.authoring.source_document_id);
    await page.getByRole("button", {name:"Fit",exact:true}).click();

    const keyboard = opened.face.subjects.find(subject=>subject.role==="Gear" && fact(opened.face,subject.identity,"kind-id")==="input/keyboard");
    const key = opened.face.relationships.filter(relation=>relation.kind==="Contains" && relation.source===keyboard.identity)
      .map(relation=>opened.face.subjects.find(subject=>subject.identity===relation.target))
      .find(subject=>subject?.role==="Port" && fact(opened.face,subject.identity,"direction")==="outgoing");
    const indicator = opened.face.subjects.find(subject=>subject.role==="Gear" && fact(opened.face,subject.identity,"kind-id")==="face/indicator");
    const sink = opened.face.relationships.filter(relation=>relation.kind==="Contains" && relation.source===indicator.identity)
      .map(relation=>opened.face.subjects.find(subject=>subject.identity===relation.target)).find(subject=>subject?.role==="Port");
    const selectKeyboard = page.getByRole("radio",{name:`Select ${keyboard.name}`,exact:true});
    await selectKeyboard.focus(); await page.keyboard.press("Space");
    await expect(page.locator("#inspector")).toBeVisible();
    await expect(page.locator(".selected-summary")).toContainText("input/keyboard");
    await expect(page.locator(".selected-summary")).toContainText("Output");
    await page.screenshot({path:info.outputPath("selected-gear.png")});
    await page.getByRole("button",{name:"Inspect",exact:true}).click();
    await expect(page.locator("#flow-root")).toHaveAttribute("aria-busy", "false");
    if (await page.locator("#palette").isVisible()) await page.getByRole("button",{name:"Library",exact:true}).click();
    await page.getByRole("button",{name:"Fit",exact:true}).click();
    await page.locator(".react-flow__edge.flow-cord").first().locator(".react-flow__edge-textwrapper").click();
    await expect(page.locator(".selected-summary")).toContainText("From");
    await expect(page.locator(".selected-summary")).toContainText("Carries");
    await page.screenshot({path:info.outputPath("selected-cord.png")});
    // Use the keyboard equivalent when the narrower Library covers a card.
    await page.getByRole("button",{name:"Inspect",exact:true}).click();
    await expect(page.locator("#flow-root")).toHaveAttribute("aria-busy", "false");
    const output = page.locator(`.faceplate-port[data-port-id="${key.identity}"]`);
    await output.focus(); await page.keyboard.press("Enter");
    const query = page.waitForResponse(response=>response.url().endsWith("/api/authoring-query") && response.request().method()==="POST");
    await page.getByRole("button",{name:"Start Cord here",exact:true}).click();
    const candidates = (await (await query).json()).candidates;
    const incompatible = candidates.find(candidate=>candidate.sink_identity===fact(opened.face,sink.identity,"semantic-id"));
    expect(incompatible.compatible).toBe(false);
    expect(incompatible.diagnostic).toBeTruthy();
    await page.getByRole("button",{name:"Inspect",exact:true}).click();
    await expect(page.locator("#flow-root")).toHaveAttribute("aria-busy", "false");
    const input = page.locator(`.faceplate-port[data-port-id="${sink.identity}"]`);
    await input.focus(); await page.keyboard.press("Enter");
    await expect(page.locator("#authoring-actions")).toContainText(incompatible.diagnostic);
    const refused = await snapshot(page);
    expect(refused.authoring.source_document_id).toBe(opened.authoring.source_document_id);
    expect(refused.authoring.source_revision).toBe(opened.authoring.source_revision);
    await page.screenshot({path:info.outputPath("invalid-connection.png")});
    await page.getByRole("button",{name:"Inspect",exact:true}).click();
    if (!await page.locator("#palette").isVisible()) await page.getByRole("button",{name:"Library",exact:true}).click();
    await page.locator("#library-query").fill("text literal");
    await expect(page.getByRole("button",{name:"Place Text literal Gear",exact:true})).toBeEnabled();
    await page.screenshot({path:info.outputPath("gear-palette.png")});
    await page.getByRole("button",{name:"Open another Plot",exact:true}).click();
    await expect(page.getByRole("button",{name:"Open Plot Interval ticker",exact:true})).toBeVisible();
    await page.reload();
    await page.getByRole("button",{name:"Open Plot Morse Network",exact:true}).click();
    await expect(page.locator(".flow-frontplate.role-gear")).toHaveCount(4);
    expect((await snapshot(page)).face.basis.body_id).toBeNull();
    expect(errors).toEqual([]);
    const manifest = await (await page.request.get(`${server.url}/patchbay.application.json`)).json();
    await writeFile(info.outputPath("application-manifest.json"),JSON.stringify(manifest,null,2));
    await writeFile(info.outputPath("receipt.json"),JSON.stringify({baseline_source:"dd92bde82017fe685404ef9197b56d3b3127006c",browser:browser.version(),resource_provenance:"application-manifest.json",viewport:{width,height:1000},face:opened.face.identity,authoring:{source_document_id:opened.authoring.source_document_id,checked_plot_id:opened.authoring.checked_plot_id,expanded_plot_id:opened.authoring.expanded_plot_id},gears:4,ports:7,cords:3,palette:opened.authoring.palette.length,refusal:incompatible.diagnostic,errors},null,2));
  } finally {
    await writeFile(info.outputPath("browser-diagnostics.json"), JSON.stringify({errors,network},null,2));
    if (errors.length) console.log(JSON.stringify({errors,lastRequests:network.slice(-12)}));
    await server.close();
  }
});


test("real Plot layouts survive refresh without editing canonical source", async ({page}, info) => {
  test.setTimeout(45_000);
  const directory = await mkdtemp(join(tmpdir(), "conduit-maker-layout-"));
  const path = join(directory,"layout.conduit");
  const source = 'plot layout {\n literal: text/literal("layout proof")\n show: face/text("Message")\n literal >> show\n}\n';
  await writeFile(path,source);
  const server = await entrance(["--plot","Layout proof",path]);
  try {
    await page.goto(server.url);
    await page.getByRole("button",{name:"Open Plot Layout proof",exact:true}).click();
    await expect(page.locator(".flow-frontplate.role-gear")).toHaveCount(2);
    const before = await snapshot(page);
    const literal = before.face.subjects.find(subject=>subject.role==="Gear" && fact(before.face,subject.identity,"kind-id")==="text/literal");
    if (await page.locator("#palette").isVisible()) await page.getByRole("button",{name:"Library",exact:true}).click();
    await page.getByRole("button",{name:"Fit",exact:true}).click();
    const node = page.locator(".react-flow__node").filter({has:page.locator(`article[data-subject-id="${literal.identity}"]`)});
    const position = () => node.evaluate(element=>{const p=new DOMMatrixReadOnly(element.style.transform);return {x:Math.trunc(p.m41),y:Math.trunc(p.m42)};});
    const original = await position();
    const layouts = page.locator("#workspace-controls");
    await layouts.locator("summary").click();
    for (const name of ["Original","Moved"]) {
      await page.getByRole("textbox",{name:"Layout name",exact:true}).fill(name);
      const save = page.getByRole("button",{name:"Save layout",exact:true});
      await save.click(); await expect(save).toBeEnabled();
      await expect(page.getByRole("combobox",{name:"Saved layouts",exact:true})).toHaveValue(name);
      await expect(page.locator("#workspace-status")).toContainText("Ready");
    }
    await layouts.locator("summary").click();
    const title = await node.locator(".faceplate-title").boundingBox();
    await page.mouse.move(title.x+title.width/2,title.y+title.height/2);await page.mouse.down();
    await page.mouse.move(title.x+title.width/2+90,title.y+title.height/2+60,{steps:8});await page.mouse.up();
    await expect.poll(position).not.toEqual(original);
    const moved = await position();
    await layouts.locator("summary").click();
    const exported = page.getByRole("textbox",{name:"Workspace document",exact:true});
    const exportButton = page.getByRole("button",{name:"Export workspace",exact:true});
    await exportButton.click();await expect(exportButton).toBeEnabled();
    const document = JSON.parse(await exported.inputValue());
    expect(document.basis.checked_plot_id).toBe(before.authoring.checked_plot_id);
    for (const [name, expected] of [["Original",original],["Moved",moved]]) {
      await page.getByRole("combobox",{name:"Saved layouts",exact:true}).selectOption(name);
      const use = page.getByRole("button",{name:"Use layout",exact:true});await use.click();await expect(use).toBeEnabled();
      await expect.poll(position).toEqual(expected);
      expect((await snapshot(page)).authoring.source_document_id).toBe(before.authoring.source_document_id);
      expect(await readFile(path,"utf8")).toBe(source);
    }
    await page.reload();await expect(page.locator(".flow-frontplate.role-gear")).toHaveCount(2);
    await expect.poll(position).toEqual(moved);
    expect((await snapshot(page)).face.basis.body_id).toBeNull();
    await page.screenshot({path:info.outputPath("layout-restored.png")});
    await writeFile(info.outputPath("workspace.json"),JSON.stringify(document,null,2));
    await writeFile(info.outputPath("source.conduit"),source);
  } finally { await server.close();await rm(directory,{recursive:true,force:true}); }
});
