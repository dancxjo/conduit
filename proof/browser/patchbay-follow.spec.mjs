import { spawn } from "node:child_process";
import { createInterface } from "node:readline";
import { expect, test } from "@playwright/test";

function startServer() {
  const process=spawn("target/debug/patchbay-html",["--documentary-fixture"],{stdio:["ignore","pipe","pipe"]});
  const errors=[];process.stderr.setEncoding("utf8");process.stderr.on("data",chunk=>errors.push(chunk));
  const lines=createInterface({input:process.stdout});
  const url=new Promise((resolve,reject)=>{lines.once("line",line=>resolve(line.replace("PATCHBAY_HTML_URL=","")));process.once("exit",code=>reject(new Error(`Patchbay HTML exited ${code}: ${errors.join("")}`)));});
  return {process,lines,url};
}

test("exact Gear realization FOLLOW crosses Program and Body then returns",async({page})=>{
  const server=startServer();
  try {
    const url=await server.url;await page.goto(url);
    const before=await(await fetch(`${url}/api/snapshot`)).json();
    await page.locator("#toggle-palette").click();
    const gear=page.locator('#subjects [data-application-component="choice-option-label"]')
      .filter({hasText:"hello/upper"}).locator('input[type="radio"][data-role="Gear"]');
    const gearIdentity=await gear.getAttribute("data-subject");
    await gear.click();
    await page.getByRole("button",{name:"Plan",exact:true}).click();
    await page.locator("#toggle-structured").click();
    await page.locator(`#structured-navigator input[type="radio"][data-subject="${gearIdentity.replaceAll('"','\\"')}"]`).click();
    const follow=page.locator("#structured-navigator").getByRole("radio",{name:/Follow Realizes to Host:/}).first();
    await expect(follow).toBeVisible();
    await follow.click();
    await expect(page.locator("#lens-label")).toHaveText("BODY · PLAN");
    const followed=await(await fetch(`${url}/api/snapshot`)).json();
    expect(followed.navigation.cursor.place).toBe("Body");
    expect(followed.navigation.cursor.focus).toMatch(/^host\//);
    expect(followed.presentation.identity).toBe(before.presentation.identity);
    expect(followed.presentation.basis).toEqual(before.presentation.basis);

    const reverse=page.locator("#structured-navigator").getByRole("radio",{name:/Follow Realizes to Gear:.*hello\/upper/}).first();
    await expect(reverse).toBeVisible();
    await reverse.click();
    await expect(page.locator("#lens-label")).toHaveText("PROGRAM · PLAN");
    const returned=await(await fetch(`${url}/api/snapshot`)).json();
    expect(returned.navigation.cursor.focus).toBe(gearIdentity);
    expect(returned.presentation.identity).toBe(before.presentation.identity);
    expect(returned.presentation.basis).toEqual(before.presentation.basis);

    await page.locator('[data-navigation-back="true"]').press("Enter");
    await expect(page.locator("#lens-label")).toHaveText("BODY · PLAN");
  } finally {
    server.lines.close();if(server.process.exitCode===null)server.process.kill("SIGTERM");
  }
});

test("a delayed navigation response cannot replace a newer cursor",async({page})=>{
  const server=startServer();
  let release;
  const held=new Promise(resolve=>{release=resolve;});
  let observed;
  const responseCaptured=new Promise(resolve=>{observed=resolve;});
  try {
    const url=await server.url;
    await page.goto(url);
    await page.route("**/api/navigation",async route=>{
      const operation=route.request().postDataJSON().operation;
      const response=await route.fetch();
      if(operation.kind==="focus"){
        observed();
        await held;
      }
      await route.fulfill({response});
    });
    await page.locator("#toggle-palette").click();
    await page.locator('#subjects [data-application-component="choice-option-label"]')
      .filter({hasText:"hello/upper"}).locator('input[type="radio"][data-role="Gear"]').click();
    await responseCaptured;
    // A second observer can advance the server while this page awaits its
    // earlier response. The page's pending controls must remain unavailable.
    await expect(page.locator("#structured-navigator")).toHaveJSProperty("inert",true);
    const basis=await(await fetch(`${url}/api/snapshot`)).json();
    const advanced=await page.request.post(`${url}/api/navigation`,{data:{
      presentation_id:basis.presentation.identity,
      presentation_revision:basis.presentation.revision,
      navigation_id:basis.navigation.navigation.identity,
      operation:{kind:"show",aspect:"Plan"},
    }});
    expect(advanced.ok()).toBe(true);
    await expect(page.locator("#lens-label")).toHaveText("PROGRAM · PLAN");
    const delayed=page.waitForResponse(response=>response.url().endsWith("/api/navigation")&&response.request().postDataJSON().operation.kind==="focus");
    release();
    await (await delayed).finished();
    await expect(page.locator("#structured-navigator")).toHaveJSProperty("inert",false);
    // Wait for the page's response callback, not another action or a retry.
    await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
    expect(await page.locator("#lens-label").textContent()).toBe("PROGRAM · PLAN");
    const current=await(await fetch(`${url}/api/snapshot`)).json();
    expect(current.navigation.cursor.aspect).toBe("Plan");
  } finally {
    release();
    server.lines.close();if(server.process.exitCode===null)server.process.kill("SIGTERM");
  }
});

test("a refused navigation response releases its pending controls",async({page})=>{
  const server=startServer();
  let release,observed;
  const held=new Promise(resolve=>{release=resolve;});
  const captured=new Promise(resolve=>{observed=resolve;});
  const errors=[];
  page.on("pageerror",error=>errors.push(error.message));
  try {
    await page.goto(await server.url);
    await page.route("**/api/navigation",async route=>{
      observed();await held;
      await route.fulfill({status:409,body:"diagnostic navigation refusal"});
    });
    await page.getByRole("button",{name:"Plan",exact:true}).click();
    await captured;
    await expect(page.locator("#structured-navigator")).toHaveJSProperty("inert",true);
    release();
    await expect(page.locator("#structured-navigator")).toHaveJSProperty("inert",false);
    await expect(page.locator("#place-controls")).toHaveJSProperty("inert",false);
    await expect(page.locator("#lens-label")).toHaveText("PROGRAM · STRUCTURE");
    expect(errors).toEqual(["navigation delivery HTTP 409"]);
  } finally {
    release();server.lines.close();if(server.process.exitCode===null)server.process.kill("SIGTERM");
  }
});
