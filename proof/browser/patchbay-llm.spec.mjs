import { spawn } from "node:child_process";
import { createInterface } from "node:readline";
import { expect, test } from "@playwright/test";

function startServer() {
  const binary=process.env.CONDUIT_PATCHBAY_HTML_BIN||"target/debug/patchbay-html";
  const processHandle=spawn(binary,["--llm-documentary-fixture"],{stdio:["ignore","pipe","pipe"]});
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

function property(snapshot,subject,name) {
  return snapshot.presentation.properties.find(item=>item.subject===subject&&item.name===name)?.value;
}

test("LLM Gear remains an ordinary typed, bounded, provenance-explicit Patchbay subject",async({page})=>{
  const server=startServer();
  try {
    const url=await server.url;
    const snapshot=await(await fetch(`${url}/api/snapshot`)).json();
    const gear=snapshot.presentation.subjects.find(subject=>subject.identity==="gear/interpreter");
    expect(gear.role).toBe("Gear");
    const ports=snapshot.presentation.relationships
      .filter(relation=>relation.source===gear.identity&&relation.kind==="Contains")
      .map(relation=>snapshot.presentation.subjects.find(subject=>subject.identity===relation.target))
      .filter(subject=>subject?.role==="Port");
    expect(ports.map(port=>port.label)).toEqual(["request","result"]);
    expect(ports.map(port=>property(snapshot,port.identity,"value-kind").Identity)).toEqual([
      "llm/interpretation-request@1","llm/interpretation-result@1",
    ]);
    expect(property(snapshot,gear.identity,"model-name").Text).toBe("gpt-oss:20b");
    expect(property(snapshot,gear.identity,"maximum-output-bytes").Count).toBe(1024);
    expect(snapshot.presentation.actions).toEqual([]);

    await page.goto(url);
    await page.getByRole("button",{name:"Subjects",exact:true}).click();
    const gearButton=page.locator('#structured-navigator input[type="radio"][data-subject="gear/interpreter"]');
    await gearButton.click();
    await expect(page.locator("#inspector .selected-summary")).toContainText("Completed");
    const pointerSelection=await page.locator("#inspector .selected-summary").innerText();
    await page.getByRole("button",{name:"Plan",exact:true}).click();
    await gearButton.click();
    await expect(page.locator("#inspector .selected-summary")).toContainText("gpt-oss:20b");

    await page.getByRole("button",{name:"Body",exact:true}).click();
    await page.getByRole("button",{name:"Structure",exact:true}).click();
    const candidate=page.locator('#structured-navigator input[type="radio"][data-subject="candidate-form/bird-dashboard"]');
    await candidate.click();
    await expect(page.locator("#inspector .selected-summary")).toContainText("OPEN AND EDITABLE");
    await expect(page.locator("#inspector .selected-summary")).toContainText("false");

    await page.getByRole("button",{name:"Program",exact:true}).click();
    const modelInfo=page.locator('#structured-navigator input[type="radio"][data-role="Info"]');
    await modelInfo.click();
    await expect(page.locator("#inspector .selected-summary")).toContainText("MODEL-DERIVED INFO");
    await page.getByRole("button",{name:"Structure",exact:true}).click();
    const proposal=page.locator('#structured-navigator input[type="radio"][data-subject="proposal/request-light"]');
    await proposal.click();
    await expect(page.locator("#inspector .selected-summary")).toContainText("AWAITING AUTHORITY");
    await page.getByRole("button",{name:"Signs",exact:true}).click();
    const systemSign=page.locator('#structured-navigator input[type="radio"][data-role="Sign"]');
    await systemSign.click();
    await expect(page.locator("#inspector .selected-summary")).toContainText("SYSTEM SIGN EVIDENCE");
    await expect(page.locator("#sign")).toContainText("SYSTEM SIGN");

    await page.getByRole("button",{name:"Structure",exact:true}).click();
    const decisions=page.locator('#structured-navigator input[type="radio"][data-subject^="decision/"]');
    await decisions.nth(0).click();
    await expect(page.locator("#inspector .selected-summary")).toContainText("ADMITTED");
    await decisions.nth(1).click();
    await expect(page.locator("#inspector .selected-summary")).toContainText("REFUSED");

    await gearButton.focus();
    await page.keyboard.press("Enter");
    await expect.poll(()=>page.locator("#inspector .selected-summary").innerText()).toBe(pointerSelection);
  } finally {
    server.process.kill("SIGTERM");
    server.lines.close();
  }
});
