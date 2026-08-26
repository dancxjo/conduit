import {test,expect} from "@playwright/test";

test("generic Presenter renders breadth and emits exact portable values without indices",async({page})=>{
  await page.goto("/proof/browser/human-interaction-presenter.test.html");
  await expect(page.getByRole("button",{name:"Panic"})).toBeVisible();
  await expect(page.getByRole("checkbox",{name:"Sustain"})).not.toBeChecked();
  await expect(page.getByRole("combobox",{name:"Waveform selection"})).toHaveValue("waveform/sine");
  await expect(page.getByRole("slider",{name:"Volume"})).toHaveValue("500000");
  await expect(page.getByRole("textbox",{name:"Instrument name"})).toBeVisible();
  await page.getByRole("combobox",{name:"Waveform selection"}).selectOption("waveform/triangle");
  const exact=await page.evaluate(()=>globalThis.proposals.at(-1));
  expect(exact.payload.values).toEqual([{value_kind:"music/waveform@1",canonical_bytes:[116,114,105,97,110,103,108,101]}]);
  expect(JSON.stringify(exact)).not.toContain("index");
});

test("semantic source and local mechanism change independently",async({page})=>{
  await page.goto("/proof/browser/human-interaction-presenter.test.html");
  await page.evaluate(()=>{const item=globalThis.authoritativeSnapshot.interactions.find(value=>value.semantic_id==="interaction/waveform");item.label="Oscillator shape";item.options.find(value=>value.identity==="waveform/triangle").label="Three-sided";globalThis.renderAuthoritative();});
  await expect(page.getByRole("heading",{name:"Oscillator shape"})).toBeVisible();
  await expect(page.getByRole("option",{name:"Three-sided"})).toHaveText("Three-sided");
  await page.evaluate(()=>{const item=globalThis.authoritativeSnapshot.interactions.find(value=>value.semantic_id==="interaction/volume");item.manifestation.scalar="number";globalThis.renderAuthoritative();});
  await expect(page.getByRole("spinbutton",{name:"Volume"})).toHaveValue("500000");
  await page.getByRole("spinbutton",{name:"Volume"}).fill("501000");await page.getByRole("spinbutton",{name:"Volume"}).press("Tab");
  expect((await page.evaluate(()=>globalThis.proposals.at(-1))).payload.values[0].value_kind).toBe("value/quantity@1");
});

test("focus draft and reload remain local while authoritative current state reconstructs",async({page})=>{
  await page.goto("/proof/browser/human-interaction-presenter.test.html");
  const name=page.getByRole("textbox",{name:"Instrument name"});await name.fill("local draft");await name.focus();
  expect(await page.evaluate(()=>globalThis.proposals.length)).toBe(0);
  expect(await page.evaluate(()=>globalThis.authoritativeSnapshot.interactions.find(value=>value.semantic_id==="interaction/name").current)).toBeUndefined();
  await page.reload();await expect(page.getByRole("combobox",{name:"Waveform selection"})).toHaveValue("waveform/sine");await expect(page.getByRole("textbox",{name:"Instrument name"})).toHaveValue("");
});

test("semantic validation and adapter failures remain explicit",async({page})=>{
  await page.goto("/proof/browser/human-interaction-presenter.test.html");
  const layers=page.getByRole("group",{name:"Active layers"});for(const box of await layers.getByRole("checkbox").all())if(await box.isChecked())await box.uncheck();
  await layers.getByRole("button",{name:"Apply layers"}).click();await expect(page.locator('[data-interaction-id="interaction/layers"] output')).toHaveText("Refused(InvalidCardinality)");
  await page.evaluate(()=>{globalThis.presenter=new (globalThis.presenter.constructor)(document.querySelector("#interactions"),{submit:async()=>{const error=new Error("lost");error.code="AdapterUnavailable";throw error;}});globalThis.presenter.render(globalThis.authoritativeSnapshot);});
  await page.getByRole("button",{name:"Panic"}).click();await expect(page.locator('[data-interaction-id="interaction/panic"] output')).toHaveText("Failed(AdapterUnavailable)");
});

test("keyboard access emits semantics while renderer trickery and pending pressure fail closed",async({page})=>{
  await page.goto("/proof/browser/human-interaction-presenter.test.html");
  await page.getByRole("button",{name:"Panic"}).focus();await page.keyboard.press("Enter");
  expect((await page.evaluate(()=>globalThis.proposals.at(-1))).payload).toEqual({kind:"activate"});
  await page.evaluate(()=>{const item=globalThis.authoritativeSnapshot.interactions.find(value=>value.semantic_id==="interaction/layers");item.options.find(value=>value.identity==="waveform/pulse").availability="unavailable";globalThis.renderAuthoritative();const checkbox=document.querySelector('[data-interaction-id="interaction/layers"] [data-option-identity="waveform/pulse"]');checkbox.disabled=false;checkbox.checked=true;});
  await page.getByRole("button",{name:"Apply layers"}).click();await expect(page.locator('[data-interaction-id="interaction/layers"] output')).toHaveText("Refused(UnavailableOption)");
  await page.evaluate(()=>{let release;globalThis.pending=new Promise(resolve=>release=resolve);globalThis.releasePending=release;globalThis.presenter=new (globalThis.presenter.constructor)(document.querySelector("#interactions"),{submit:async()=>{await globalThis.pending;return {disposition:"Accepted"};}});globalThis.presenter.render(globalThis.authoritativeSnapshot);});
  await page.getByRole("button",{name:"Panic"}).click({noWaitAfter:true});
  await page.getByRole("checkbox",{name:"Sustain"}).check();
  await expect(page.locator('[data-interaction-id="interaction/sustain"] output')).toHaveText("Refused(QueuePressure)");
  await page.evaluate(()=>{globalThis.presenter.cancelPending();globalThis.releasePending();});
  await expect(page.locator('[data-interaction-id="interaction/panic"] output')).toHaveText("Failed(Cancelled)");
});
