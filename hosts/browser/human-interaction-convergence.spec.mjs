import {test,expect} from "@playwright/test";

test.beforeEach(async({page})=>{await page.goto("/hosts/browser/human-interaction-convergence.test.html");});

test("physical and browser mechanisms converge only through semantic state",async({page})=>{
  await page.evaluate(()=>window.capstone.physicalChoice("saw"));
  await expect(page.getByLabel("waveform")).toHaveValue("waveform/saw");
  await page.getByLabel("waveform").selectOption("waveform/triangle");
  await expect(page.locator("#physical")).toContainText('"waveform":"triangle"');

  await page.evaluate(()=>window.capstone.physicalScalar(2000));
  await expect(page.getByLabel("volume")).toHaveValue("50");
  await page.getByLabel("volume").fill("73");await page.getByLabel("volume").press("Tab");
  await expect(page.locator("#physical")).toContainText('"volume":73');

  await page.evaluate(()=>window.capstone.physicalPanic());
  await page.getByRole("button",{name:"Panic"}).click();
  expect(await page.evaluate(()=>window.capstone.state.panicCount)).toBe(2);
  expect(await page.evaluate(()=>window.capstone.evidence.map(item=>item.semantic))).toEqual(["waveform","waveform","volume","volume","panic","panic"]);
});

test("boolean and browser-only bounded text remain honest",async({page})=>{
  await page.getByLabel("sustain").check();
  await expect(page.locator("#physical")).toContainText('"sustain":true');
  await page.getByLabel("name").fill("Still Conduit");
  await page.getByRole("button",{name:"Set name"}).click();
  expect(await page.evaluate(()=>window.capstone.state.name)).toBe("Still Conduit");
  expect(await page.evaluate(()=>window.capstone.physicalCapabilities.textEntry)).toBe(false);
});

test("either Presenter can be removed independently",async({page})=>{
  await page.evaluate(()=>window.capstone.detachBrowser());
  await page.evaluate(()=>window.capstone.physicalChoice("pulse"));
  expect(await page.evaluate(()=>window.capstone.state.waveform)).toBe("pulse");
  await page.evaluate(()=>{window.capstone.attachBrowser();window.capstone.detachPhysical();});
  await page.getByLabel("waveform").selectOption("waveform/sine");
  expect(await page.evaluate(()=>window.capstone.state.waveform)).toBe("sine");
  await expect(page.locator("#physical")).toHaveText("detached");
});

test("evidence keeps proof and realization identities bounded",async({page})=>{
  await page.evaluate(()=>window.capstone.physicalScalar(2900));
  const evidence=await page.evaluate(()=>window.capstone.evidence[0]);
  expect(evidence).toEqual({source_document_id:"cb0dcd832852f396cb3ea376beb888a6b9991660d98de65da3f06bc9c0040693",checked_form_id:"ebffdd1f07a8fe360ea35f948ff1f63ecd3bc816d8aa1322b96dfa0162410d60",expanded_form_id:"3887b339771251500403daa481137280964bbcdb86a98fdce76b655f0631e681",body_id:"body/human-interaction-capstone@1",plan_id:"plan/human-interaction-capstone@1",play_id:"play/human-interaction-capstone@1",source:"physical",semantic:"volume",resulting_revision:2,mapping_identity:"mapping/pico-adc12-percent"});
});
