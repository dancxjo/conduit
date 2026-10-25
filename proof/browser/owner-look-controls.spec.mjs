import { expect, test } from '@playwright/test';

// Actual controls and CSS with supplied component evidence; not an owner receipt.
const shown = { component: { recipe_identity: 'conduit.face/todo-grouped@1',
  expression: { expression: { density: 'compact', palette: 'phosphor', text_scale_percent: 200 },
    accessibility_overrides: { text_scale: true } } } };
async function mount(page, view = shown) {
  await page.goto('/proof/browser/signal-dom-host.test.html');
  await page.evaluate(async view => {
    const { createOwnerLookControls } = await import('/targets/browser/handbook/owner-look-controls.mjs');
    document.body.innerHTML = '<main class="handbook-application"></main>';
    const style = document.createElement('link'); style.rel = 'stylesheet';
    style.href = '/targets/browser/handbook/application.css'; document.head.append(style);
    globalThis.calls = [];
    globalThis.lookControls = createOwnerLookControls(selection => calls.push(selection));
    lookControls.observe(view); lookControls.setDisabled(false);
    document.querySelector('main').append(lookControls.element);
    globalThis.originalView = view;
  }, view);
}

test('independent palette and density proposals retain checked layout and hard text minimum', async ({ page }) => {
  await mount(page);
  await expect(page.getByLabel('Layout', { exact: true })).toHaveValue('grouped');
  await expect(page.getByLabel('Minimum text size', { exact: true })).toHaveValue('200');
  for (const [density, palette, look] of [['compact', 'editorial', 'compact-editorial'],
    ['spacious', 'editorial', 'spacious'], ['spacious', 'phosphor', 'spacious-phosphor'],
    ['compact', 'phosphor', 'compact']]) {
    await page.getByLabel('Density', { exact: true }).selectOption(density);
    await page.getByLabel('Palette', { exact: true }).selectOption(palette);
    await page.getByRole('button', { name: 'Apply face', exact: true }).click();
    expect(await page.evaluate(() => calls.at(-1))).toEqual({ look, recipe: 'grouped', minimumTextScalePercent: 200 });
  }
  expect(await page.evaluate(() => originalView)).toEqual(shown);
  await page.evaluate(() => lookControls.setDisabled(true));
  await expect(page.getByLabel('Palette', { exact: true })).toBeDisabled();
  await page.getByRole('button', { name: 'Apply face', exact: true }).evaluate(node => node.dispatchEvent(new Event('click')));
  expect(await page.evaluate(() => calls.length)).toBe(4);
});

test('foreign or unavailable recipe cannot silently become a different component', async ({ page }) => {
  await mount(page, { component: { ...shown.component, recipe_identity: 'foreign/grouped@1' } });
  await page.getByRole('button', { name: 'Apply face', exact: true }).click();
  expect(await page.evaluate(() => calls)).toEqual([]);
  await expect(page.getByRole('status')).toHaveText('Choose a supported appearance and layout.');
  await page.getByLabel('Layout', { exact: true }).selectOption('flat');
  await page.getByRole('button', { name: 'Apply face', exact: true }).focus();
  await page.keyboard.press('Enter');
  expect(await page.evaluate(() => calls)).toEqual([{ look: 'compact', recipe: 'flat', minimumTextScalePercent: 200 }]);
});

test('Look controls remain readable and reachable at narrow width and large forced-color text', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 900 });
  await page.emulateMedia({ forcedColors: 'active', reducedMotion: 'reduce' });
  await mount(page);
  await page.evaluate(() => {
    document.documentElement.style.fontSize = '200%';
    document.body.style.margin = '0';
  });
  for (const name of ['Density', 'Palette', 'Layout', 'Minimum text size']) {
    const control = page.getByLabel(name, { exact: true });
    expect((await control.boundingBox()).height).toBeGreaterThanOrEqual(44);
  }
  expect(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth)).toBe(false);
  const palette = page.getByLabel('Palette', { exact: true });
  await page.getByLabel('Density', { exact: true }).focus();
  await page.keyboard.press('Tab');
  await expect(palette).toBeFocused();
  expect(await palette.evaluate(node => getComputedStyle(node).outlineStyle)).toBe('solid');
});

test('fresh Faces preserve unfinished Look proposals until the acknowledged policy changes', async ({ page }) => {
  const generic={body_id:'fixture/body',face_id:'fixture/face',face_revision:1,show_id:'show/1'};
  await mount(page,generic);
  await page.getByLabel('Density',{exact:true}).selectOption('spacious');
  await page.getByLabel('Palette',{exact:true}).selectOption('editorial');
  await page.getByLabel('Layout',{exact:true}).selectOption('grouped');
  await page.getByLabel('Minimum text size',{exact:true}).selectOption('150');
  const layout=page.getByLabel('Layout',{exact:true});await layout.focus();
  await page.evaluate(view=>lookControls.observe({...view,face_revision:2,show_id:'show/2'}),generic);
  await expect(layout).toBeFocused();
  await expect(layout).toHaveValue('grouped');
  await expect(page.getByLabel('Density',{exact:true})).toHaveValue('spacious');
  await expect(page.getByLabel('Palette',{exact:true})).toHaveValue('editorial');
  await expect(page.getByLabel('Minimum text size',{exact:true})).toHaveValue('150');
  await page.getByRole('button',{name:'Apply face',exact:true}).click();
  expect(await page.evaluate(()=>calls)).toEqual([{look:'spacious',recipe:'grouped',minimumTextScalePercent:150}]);
  await page.evaluate(view=>lookControls.observe({...view,body_id:'fixture/body'}),shown);
  await expect(page.getByLabel('Density',{exact:true})).toHaveValue('compact');
  await expect(page.getByLabel('Palette',{exact:true})).toHaveValue('phosphor');
  await expect(layout).toHaveValue('grouped');
  await expect(page.getByLabel('Minimum text size',{exact:true})).toHaveValue('200');
});


test('Patchbay appearance retains its node recipe across every Look and fresh Show', async ({ page }, info) => {
  await mount(page, { body_id: 'patchbay/body', component: { ...shown.component,
    recipe_identity: 'conduit.face/patchbay-nodes@1' } });
  await expect(page.getByLabel('Layout', { exact: true })).toHaveValue('patchbay-nodes');
  for (const [density, palette, look] of [['compact', 'editorial', 'compact-editorial'],
    ['spacious', 'editorial', 'spacious'], ['spacious', 'phosphor', 'spacious-phosphor'],
    ['compact', 'phosphor', 'compact']]) {
    await page.getByLabel('Density', { exact: true }).selectOption(density);
    await page.getByLabel('Palette', { exact: true }).selectOption(palette);
    await page.getByRole('button', { name: 'Apply face', exact: true }).focus();
    await page.keyboard.press('Enter');
    expect(await page.evaluate(() => calls.at(-1))).toEqual({ look,
      recipe: 'patchbay-nodes', minimumTextScalePercent: 200 });
  }
  await page.evaluate(view => lookControls.observe({ body_id: 'patchbay/body',
    show_id: 'fresh/show', component: { ...view.component,
      recipe_identity: 'conduit.face/patchbay-nodes@1' } }), shown);
  await expect(page.getByLabel('Layout', { exact: true })).toHaveValue('patchbay-nodes');
  await page.screenshot({ path: info.outputPath('patchbay-appearance-controls.png') });
});

test('history units and appearance axes make independent owner proposals', async ({page}, info) => {
  const view = {body_id:'history/body',subjects:[{identity:'environment-history',role:'Collection'}],
    component:{recipe_identity:'conduit.face/environment-history@1',history:{temperature_unit:'°C'},
      expression:{expression:{density:'compact',palette:'editorial',text_scale_percent:100}}}};
  await mount(page,view);
  for (const [unit,density,palette] of [['kelvin','spacious','phosphor'],['fahrenheit','compact','phosphor'],['celsius','spacious','editorial']]) {
    await page.getByLabel('Temperature unit',{exact:true}).selectOption(unit);
    await page.getByLabel('Density',{exact:true}).selectOption(density);
    await page.getByLabel('Palette',{exact:true}).selectOption(palette);
    await page.getByRole('button',{name:'Apply face',exact:true}).click();
    expect(await page.evaluate(() => calls.at(-1))).toEqual({recipe:'environment-history',unit,density,palette,minimumTextScalePercent:100});
  }
  await page.evaluate(view => lookControls.observe({...view,show_id:'fresh'}),view);
  await expect(page.getByLabel('Temperature unit',{exact:true})).toHaveValue('celsius');
  await expect(page.getByLabel('Density',{exact:true})).toHaveValue('spacious');
  await page.screenshot({path:info.outputPath('history-unit-controls.png')});
});

test('fresh identical history policy preserves unfinished unit and visual proposals', async ({page}) => {
  const view = {body_id:'history/body',face_id:'face/one',face_revision:1,show_id:'show/one',
    subjects:[{identity:'environment-history',role:'Collection'}],
    component:{recipe_identity:'conduit.face/environment-history@1',history:{temperature_unit:'°C'},
      expression:{expression:{density:'compact',palette:'editorial',text_scale_percent:100}}}};
  await mount(page,view);
  const unit = page.getByLabel('Temperature unit',{exact:true});
  await unit.selectOption('kelvin');
  await page.getByLabel('Density',{exact:true}).selectOption('spacious');
  await page.getByLabel('Palette',{exact:true}).selectOption('phosphor');
  await unit.focus();
  await page.evaluate(view => lookControls.observe({...view,face_revision:2,show_id:'show/two'}),view);
  await expect(unit).toHaveValue('kelvin');
  await expect(unit).toBeFocused();
  await expect(page.getByLabel('Density',{exact:true})).toHaveValue('spacious');
  await expect(page.getByLabel('Palette',{exact:true})).toHaveValue('phosphor');
  await page.getByRole('button',{name:'Apply face',exact:true}).click();
  expect(await page.evaluate(() => calls.at(-1))).toEqual({recipe:'environment-history',unit:'kelvin',density:'spacious',palette:'phosphor',minimumTextScalePercent:100});
  await page.evaluate(view => lookControls.observe({...view,component:{...view.component,history:{temperature_unit:'°F'}}}),view);
  await expect(unit).toHaveValue('fahrenheit');
});
