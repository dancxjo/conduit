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

test("graduated Body opens as Program, Body, and readable History",async({page})=>{
  const server=startServer();
  try {
    const url=await server.url;await page.goto(url);
    const snapshot=await(await fetch(`${url}/api/snapshot`)).json();
    await expect(page.locator("#body-name")).toHaveText("Roseau");
    await expect(page.locator("#body-program")).toHaveText("Hello");
    await expect(page.locator("#body-status")).toContainText("3 Parts");
    await expect(page.locator("#body-status")).toContainText("2 current Hosts");
    await expect(page.locator("#body-placement")).toContainText("Hosted by this Body");
    await expect(page.locator("#body-lifecycle-action")).toBeEnabled();
    await expect(page.locator("#body-lifecycle-action")).toHaveText("Hold");
    await expect(page.locator("#workbench-places")).toHaveText(/Program.*Body.*History/);
    await expect(page.locator("body")).toHaveAttribute("data-workbench-view","program");
    await expect(page.locator("#flow-root .react-flow")).toBeVisible();

    await page.locator("#show-body").click();
    await expect(page.locator("body")).toHaveAttribute("data-workbench-view","body");
    await expect(page.locator("#parts")).toBeVisible();
    await expect(page.locator("#parts-title")).toContainText("Roseau · 3 Parts");
    await expect(page.getByRole("list",{name:"Body Parts"}).getByRole("listitem")).toHaveCount(3);
    await page.getByRole("list",{name:"Body Parts"}).getByRole("button",{name:"Inspect"}).first().click();
    await expect(page.locator("#parts-details")).toContainText("Host");
    await expect(page.locator("#parts-details")).toContainText("Boot");
    let current=await(await fetch(`${url}/api/snapshot`)).json();
    expect(current.navigation.cursor.place).toBe("Body");
    expect(current.navigation.cursor.aspect).toBe("Structure");

    await page.locator("#show-history").click();
    await expect(page.locator("body")).toHaveAttribute("data-workbench-view","history");
    await expect(page.locator("#history-workspace")).toBeVisible();
    await expect(page.locator("#history-entries .history-entry")).toHaveCount(7);
    await expect(page.locator("#history-entries")).toContainText("Born");
    await expect(page.locator("#history-entries")).toContainText("Part admitted");
    await expect(page.locator("#history-entries")).toContainText("Host joined");
    await expect(page.locator("#history-entries")).toContainText("Graduated from the Crèche");
    const historyText=await page.locator("#history-entries").innerText();
    expect(historyText).not.toMatch(/\bToday\b|\bAM\b|\bPM\b|\d{1,2}:\d{2}/);
    await page.locator("#history-entries details").last().locator("summary").click();
    await expect(page.locator("#history-entries details").last()).toContainText("Sign");
    await expect(page.locator("#history-entries details").last()).toContainText("Graduated");
    await page.locator("#history-linear summary").click();
    await expect(page.locator("#history-linear li")).toHaveCount(7);
    current=await(await fetch(`${url}/api/snapshot`)).json();
    expect(current.navigation.cursor.place).toBe("Body");
    expect(current.navigation.cursor.aspect).toBe("Signs");
    expect(current.presentation.identity).toBe(snapshot.presentation.identity);

    await page.locator("#toggle-truth").click();
    await expect(page.locator("#deep-inspection")).toBeVisible();
    await expect(page.getByRole("heading",{name:"Exact Plan"})).toBeVisible();
    const exactClose=page.waitForResponse(response=>response.url().endsWith("/api/navigation")&&response.request().method()==="POST");
    await page.keyboard.press("Escape");
    await exactClose;
    const programNavigation=page.waitForResponse(response=>response.url().endsWith("/api/navigation")&&response.request().method()==="POST");
    await page.locator("#show-program").click();
    await programNavigation;
    await expect(page.locator("body")).toHaveAttribute("data-workbench-view","program");
    await expect(page.locator("#flow-root .react-flow")).toBeVisible();
    current=await(await fetch(`${url}/api/snapshot`)).json();
    expect(current.navigation.cursor.place).toBe("Program");
  } finally {server.lines.close();if(server.process.exitCode===null)server.process.kill("SIGTERM");}
});

test("malformed or detached workbench cannot leave old friendly Body content",async({page})=>{
  const server=startServer();
  try {
    const url=await server.url;await page.goto(url);
    const snapshot=await(await fetch(`${url}/api/snapshot`)).json();
    await expect(page.locator("#body-name")).toHaveText("Roseau");
    snapshot.workbench.history.entries[1].evidence_sequence=1;
    await page.route("**/api/snapshot",route=>route.fulfill({status:200,contentType:"application/json",body:JSON.stringify(snapshot)}));
    await page.evaluate(()=>window.patchbayReload());
    await expect(page.locator("#body-name")).toHaveText("Body unavailable");
    await expect(page.locator("#body-status")).toContainText("Body evidence unavailable");
    await expect(page.locator("#show-body")).toBeDisabled();
    await expect(page.locator("#show-history")).toBeDisabled();
    await expect(page.locator("#history-entries .history-entry")).toHaveCount(0);
  } finally {server.lines.close();if(server.process.exitCode===null)server.process.kill("SIGTERM");}
});
