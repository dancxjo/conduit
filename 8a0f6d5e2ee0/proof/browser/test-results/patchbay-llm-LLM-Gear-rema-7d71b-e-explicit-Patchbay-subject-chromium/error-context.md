# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: patchbay-llm.spec.mjs >> LLM Gear remains an ordinary typed, bounded, provenance-explicit Patchbay subject
- Location: proof/browser/patchbay-llm.spec.mjs:23:1

# Error details

```
Error: expect(received).toEqual(expected) // deep equality

- Expected  - 2
+ Received  + 2

  Array [
-   "request",
-   "result",
+   undefined,
+   undefined,
  ]
```

# Test source

```ts
  1  | import { spawn } from "node:child_process";
  2  | import { createInterface } from "node:readline";
  3  | import { expect, test } from "@playwright/test";
  4  | 
  5  | function startServer() {
  6  |   const binary=process.env.CONDUIT_PATCHBAY_HTML_BIN||"target/debug/patchbay-html";
  7  |   const processHandle=spawn(binary,["--llm-documentary-fixture"],{stdio:["ignore","pipe","pipe"]});
  8  |   const errors=[];
  9  |   processHandle.stderr.setEncoding("utf8");
  10 |   processHandle.stderr.on("data",chunk=>errors.push(chunk));
  11 |   const lines=createInterface({input:processHandle.stdout});
  12 |   const url=new Promise((resolve,reject)=>{
  13 |     lines.once("line",line=>resolve(line.replace("PATCHBAY_HTML_URL=","")));
  14 |     processHandle.once("exit",code=>reject(new Error(`Patchbay HTML exited ${code}: ${errors.join("")}`)));
  15 |   });
  16 |   return {process:processHandle,lines,url};
  17 | }
  18 | 
  19 | function property(snapshot,subject,name) {
  20 |   return snapshot.presentation.properties.find(item=>item.subject===subject&&item.name===name)?.value;
  21 | }
  22 | 
  23 | test("LLM Gear remains an ordinary typed, bounded, provenance-explicit Patchbay subject",async({page})=>{
  24 |   const server=startServer();
  25 |   try {
  26 |     const url=await server.url;
  27 |     const snapshot=await(await fetch(`${url}/api/snapshot`)).json();
  28 |     const gear=snapshot.presentation.subjects.find(subject=>subject.identity==="gear/interpreter");
  29 |     expect(gear.role).toBe("Gear");
  30 |     const ports=snapshot.presentation.relationships
  31 |       .filter(relation=>relation.source===gear.identity&&relation.kind==="Contains")
  32 |       .map(relation=>snapshot.presentation.subjects.find(subject=>subject.identity===relation.target))
  33 |       .filter(subject=>subject?.role==="Port");
> 34 |     expect(ports.map(port=>port.label)).toEqual(["request","result"]);
     |                                         ^ Error: expect(received).toEqual(expected) // deep equality
  35 |     expect(ports.map(port=>property(snapshot,port.identity,"value-kind").Identity)).toEqual([
  36 |       "llm/interpretation-request@1","llm/interpretation-result@1",
  37 |     ]);
  38 |     expect(property(snapshot,gear.identity,"model-name").Text).toBe("gpt-oss:20b");
  39 |     expect(property(snapshot,gear.identity,"maximum-output-bytes").Count).toBe(1024);
  40 |     expect(snapshot.presentation.actions).toEqual([]);
  41 | 
  42 |     await page.goto(url);
  43 |     await page.getByRole("button",{name:"Form",exact:true}).click();
  44 |     await page.locator("#toggle-inspector").click();
  45 |     await page.locator("#structured-navigator").evaluate(element=>{element.closest("details").open=true;});
  46 |     const gearButton=page.locator('#structured-navigator input[type="radio"][data-subject="gear/interpreter"]');
  47 |     await gearButton.click();
  48 |     await expect(page.locator("#inspector .selected-summary")).toContainText("Completed");
  49 |     const pointerSelection=await page.locator("#inspector .selected-summary").innerText();
  50 |     await page.getByRole("button",{name:"Realization",exact:true}).click();
  51 |     await gearButton.click();
  52 |     await expect(page.locator("#inspector .selected-summary")).toContainText("gpt-oss:20b");
  53 | 
  54 |     await page.getByRole("button",{name:"Body",exact:true}).click();
  55 |     await page.getByRole("button",{name:"Structure",exact:true}).click();
  56 |     const candidate=page.locator('#structured-navigator input[type="radio"][data-subject="candidate-form/bird-dashboard"]');
  57 |     await candidate.click();
  58 |     await expect(page.locator("#inspector .selected-summary")).toContainText("OPEN AND EDITABLE");
  59 |     await expect(page.locator("#inspector .selected-summary")).toContainText("false");
  60 | 
  61 |     await page.getByRole("button",{name:"Form",exact:true}).click();
  62 |     const modelInfo=page.locator('#structured-navigator input[type="radio"][data-role="Info"]');
  63 |     await modelInfo.click();
  64 |     await expect(page.locator("#inspector .selected-summary")).toContainText("MODEL-DERIVED INFO");
  65 |     await page.getByRole("button",{name:"Structure",exact:true}).click();
  66 |     const proposal=page.locator('#structured-navigator input[type="radio"][data-subject="proposal/request-light"]');
  67 |     await proposal.click();
  68 |     await expect(page.locator("#inspector .selected-summary")).toContainText("AWAITING AUTHORITY");
  69 |     await page.getByRole("button",{name:"Debug",exact:true}).click();
  70 |     const systemSign=page.locator('#structured-navigator input[type="radio"][data-role="Sign"]');
  71 |     await systemSign.click();
  72 |     await expect(page.locator("#inspector .selected-summary")).toContainText("SYSTEM SIGN EVIDENCE");
  73 |     await expect(page.locator("#sign")).toContainText("SYSTEM SIGN");
  74 | 
  75 |     await page.getByRole("button",{name:"Structure",exact:true}).click();
  76 |     const decisions=page.locator('#structured-navigator input[type="radio"][data-subject^="decision/"]');
  77 |     await decisions.nth(0).click();
  78 |     await expect(page.locator("#inspector .selected-summary")).toContainText("ADMITTED");
  79 |     await decisions.nth(1).click();
  80 |     await expect(page.locator("#inspector .selected-summary")).toContainText("REFUSED");
  81 | 
  82 |     await page.getByRole("button",{name:"Form",exact:true}).click();
  83 |     await page.getByRole("button",{name:"Structure",exact:true}).click();
  84 |     await expect.poll(async()=>{const cursor=((await(await fetch(`${url}/api/snapshot`)).json()).navigation.cursor);return `${cursor.place}/${cursor.aspect}`;}).toBe("Program/Structure");
  85 |     if(await page.locator("body").getAttribute("data-inspector-open")!=="true"){await page.locator("#toggle-inspector").click();await expect.poll(async()=>((await(await fetch(`${url}/api/snapshot`)).json()).navigation.cursor.depth)).toBe("Detail");}
  86 |     await page.locator("#structured-navigator").evaluate(element=>{element.closest("details").open=true;});
  87 |     const keyboardGear=page.locator('#structured-navigator input[type="radio"][data-subject="gear/interpreter"]');
  88 |     await expect(keyboardGear).toBeVisible();
  89 |     await keyboardGear.focus();
  90 |     await page.keyboard.press("Space");
  91 |     await expect.poll(()=>page.locator("#inspector .selected-summary").innerText()).toBe(pointerSelection);
  92 |   } finally {
  93 |     server.process.kill("SIGTERM");
  94 |     server.lines.close();
  95 |   }
  96 | });
  97 | 
```