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

test("Inspector and Exact truth enact portable Depth and Back",async({page})=>{
  const server=startServer();
  try {
    const url=await server.url;await page.goto(url);
    const original=await(await fetch(`${url}/api/snapshot`)).json();
    await page.locator("#toggle-palette").click();
    const gear=page.locator('#subjects input[type="radio"][data-role="Gear"]').first();
    const identity=await gear.getAttribute("data-subject");
    await gear.click();
    const detail=await(await fetch(`${url}/api/snapshot`)).json();
    expect(detail.navigation.cursor.focus).toBe(identity);
    expect(detail.navigation.cursor.depth).toBe("Detail");
    expect(detail.interaction.revision).toBe(original.interaction.revision+1);
    await expect(page.locator("body")).toHaveAttribute("data-inspector-open","true");
    await expect(page.locator("#inspector .selected-summary")).not.toContainText("source-port");

    await page.locator("#toggle-truth").click();
    const exact=await(await fetch(`${url}/api/snapshot`)).json();
    expect(exact.navigation.cursor.depth).toBe("Exact");
    expect(exact.presentation.identity).toBe(original.presentation.identity);
    expect(exact.presentation.basis).toEqual(original.presentation.basis);
    await expect(page.locator("#deep-inspection")).toBeVisible();

    await page.keyboard.press("Escape");
    await expect(page.locator("#deep-inspection")).toBeHidden();
    expect((await(await fetch(`${url}/api/snapshot`)).json()).navigation.cursor.depth).toBe("Detail");
    await page.locator("#toggle-inspector").click();
    await expect(page.locator("#inspector")).toBeVisible();
    await expect.poll(async()=>(await(await fetch(`${url}/api/snapshot`)).json()).navigation.cursor.depth).toBe("Detail");
    await page.locator("#toggle-inspector").click();
    const returned=await(await fetch(`${url}/api/snapshot`)).json();
    expect(returned.navigation.cursor.depth).toBe(original.navigation.cursor.depth);
    expect(returned.navigation.cursor.focus).toBe(original.navigation.cursor.focus);
  } finally {
    server.lines.close();if(server.process.exitCode===null)server.process.kill("SIGTERM");
  }
});
