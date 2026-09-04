import { spawn } from "node:child_process";
import { createInterface } from "node:readline";
import { expect, test } from "@playwright/test";

function startStage(stage) {
  const binary=process.env.CONDUIT_PATCHBAY_HTML_BIN||"target/debug/patchbay-html";
  const processHandle=spawn(binary,["--llm-embodiment-fixture",String(stage)],{stdio:["ignore","pipe","pipe"]});
  const errors=[];
  processHandle.stderr.setEncoding("utf8");
  processHandle.stderr.on("data",chunk=>errors.push(chunk));
  const lines=createInterface({input:processHandle.stdout});
  const url=new Promise((resolve,reject)=>{
    lines.once("line",line=>resolve(line.replace("PATCHBAY_HTML_URL=","")));
    processHandle.once("exit",code=>reject(new Error(`Patchbay HTML exited ${code}: ${errors.join("")}`)));
  });
  return {process:processHandle,lines,url};
}

function subjects(snapshot,role) {
  return snapshot.presentation.subjects.filter(subject=>subject.role===role);
}

test("the same model gains expression and protected power only through exact Form Cords",async({page})=>{
  const snapshots=[];
  for (const stage of [0,1,2]) {
    const server=startStage(stage);
    try {
      const url=await server.url;
      snapshots.push(await(await fetch(`${url}/api/snapshot`)).json());
      if (stage===2) {
        await page.goto(url);
        await page.getByRole("button",{name:"Signs",exact:true}).click();
        await page.getByRole("button",{name:"Subjects",exact:true}).click();
        await page.locator('#structured-navigator input[type="radio"][data-role="Sign"]').click();
        await expect(page.locator("#inspector .selected-summary")).toContainText("SYSTEM SIGN EVIDENCE");
      }
    } finally {
      server.process.kill("SIGTERM");
      server.lines.close();
    }
  }

  const implementations=snapshots.map(snapshot=>snapshot.presentation.properties
    .find(property=>property.name==="implementation-id")?.value.Identity);
  expect(new Set(implementations).size).toBe(1);
  expect(implementations[0]).toBe("ollama/gpt-oss:20b/exact-digest");
  expect(snapshots.map(snapshot=>subjects(snapshot,"Form").length)).toEqual([1,1,1]);
  expect(snapshots.map(snapshot=>subjects(snapshot,"Cord").length)).toEqual([2,3,4]);
  expect(snapshots.map(snapshot=>subjects(snapshot,"Sign").length)).toEqual([0,0,1]);
  const decisions=snapshots.map(snapshot=>snapshot.presentation.properties
    .find(property=>property.name==="authority-state")?.value.Text);
  expect(decisions).toEqual(["REFUSED","REFUSED","ADMITTED"]);
});
