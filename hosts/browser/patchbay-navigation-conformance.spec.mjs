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

async function expectCurrentObservation(page,url) {
  const snapshot=await(await fetch(`${url}/api/snapshot`)).json();
  const expected=await(await fetch(`${url}/api/navigation-observation`)).json();
  const actual=await page.evaluate(async value=>(await import("/assets/portable-navigation.js")).observeCurrent(value),snapshot);
  expect(actual).toEqual(expected);
  return snapshot;
}

test("browser projection agrees exactly with the portable navigation observation",async({page})=>{
  const server=startServer();
  try {
    const url=await server.url;await page.goto(url);
    const initial=await expectCurrentObservation(page,url);
    const stale=structuredClone(initial);stale.navigation.cursor.revision+=1;
    await expect(page.evaluate(async value=>(await import("/assets/portable-navigation.js")).observeCurrent(value),stale)).rejects.toThrow("stale portable navigation identity");

    await page.locator("#toggle-palette").click();
    await page.locator('#subjects button[data-role="Gear"]').first().click();
    const focused=await expectCurrentObservation(page,url);
    expect(focused.navigation.cursor.focus).toMatch(/^gear\//);
    expect(focused.navigation.cursor.depth).toBe("Detail");

    await page.locator("#toggle-structured").click();
    await page.locator("#structured-navigator [data-follow]").filter({hasText:"Host:"}).first().click();
    const followed=await expectCurrentObservation(page,url);
    expect(followed.navigation.cursor.place).toBe("Body");
    expect(followed.navigation.cursor.aspect).toBe("Plan");
    expect(followed.navigation.cursor.focus).toMatch(/^host\//);
    expect(followed.presentation.identity).toBe(initial.presentation.identity);
    expect(followed.presentation.basis).toEqual(initial.presentation.basis);
  } finally {
    server.lines.close();if(server.process.exitCode===null)server.process.kill("SIGTERM");
  }
});
