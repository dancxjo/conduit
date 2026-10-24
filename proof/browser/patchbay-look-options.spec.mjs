import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import { writeFile } from 'node:fs/promises';
import { expect, test } from '@playwright/test';

// Documentary graph: checks appearance and retained renderer identities, not
// live Body execution or native Mask selection.
test('three typed application Looks retain the exact Face and graph', async ({ page }, info) => {
  const child=spawn('target/debug/conduit-browser-patchbay-workbench',['--documentary-fixture'],{stdio:['ignore','pipe','pipe']});
  const errors=[];child.stderr.on('data',chunk=>errors.push(String(chunk)));
  const lines=createInterface({input:child.stdout});
  const url=await new Promise((resolve,reject)=>{
    lines.once('line',line=>resolve(line.replace('PATCHBAY_HTML_URL=','')));
    child.once('exit',code=>reject(new Error(`server ${code}: ${errors.join('')}`)));
  });
  try {
    await page.setViewportSize({width:1440,height:1000});
    await page.goto(url);
    await expect(page.locator('body')).toHaveAttribute('data-application-ready','true');
    await page.getByRole('button',{name:'Plot',exact:true}).click();
    await expect(page.locator('#lens-label')).toHaveText('PLOT · STRUCTURE');
    await expect(page.locator('.flow-frontplate').first()).toBeVisible();
    await page.locator('#fit-flow').click();
    const baseline=await (await page.request.get(`${url}/api/snapshot`)).json();
    const nodes=await page.locator('.flow-frontplate').evaluateAll(nodes=>nodes.map(node=>node.dataset.subject));
    const picker=page.getByRole('combobox',{name:'Look',exact:true});
    await picker.focus();
    for(const [name,background,font] of [
      ['phosphor','rgb(5, 7, 11)','Georgia'],
      ['editorial','rgb(244, 240, 231)','Georgia'],
      ['engineering','rgb(234, 240, 246)','Inter'],
    ]) {
      if(name!=='phosphor')await page.keyboard.press('ArrowDown');
      await expect(picker).toHaveValue(`conduit.look/${name}@1`);
      await expect(picker).toBeFocused();
      await expect(page.locator('body')).toHaveCSS('background-color',background);
      const bodyFont=await page.locator('body').evaluate(node=>getComputedStyle(node).fontFamily);
      expect(bodyFont).toContain(font);
      expect(await page.locator('.flow-frontplate').evaluateAll(nodes=>nodes.map(node=>node.dataset.subject))).toEqual(nodes);
      const current=await (await page.request.get(`${url}/api/snapshot`)).json();
      expect(current.face).toEqual(baseline.face);
      await page.screenshot({path:info.outputPath(`look-${name}.png`),fullPage:true,animations:'disabled'});
    }
    await page.setViewportSize({width:390,height:844});
    await page.emulateMedia({forcedColors:'active',reducedMotion:'reduce'});
    await page.addStyleTag({content:'html { font-size:32px; }'});
    // The narrow action tray scrolls horizontally. Reach the picker through
    // the real Tab sequence so the browser reveals its focused control.
    await page.locator('#arrange').focus();
    await page.keyboard.press('Tab');
    await expect(picker).toBeFocused();
    for(const name of ['phosphor','editorial','engineering']) {
      await picker.selectOption(`conduit.look/${name}@1`);
      await picker.focus();
      await expect(picker).toBeFocused();
      expect(await picker.evaluate(node=>node.scrollWidth<=node.clientWidth+1)).toBe(true);
      const box=await picker.boundingBox();
      expect(box.x).toBeGreaterThanOrEqual(0);
      expect(box.x+box.width).toBeLessThanOrEqual(391);
      await expect(page.locator('body')).toHaveAttribute('data-application-look',`conduit.look/${name}@1`);
    }
    await page.screenshot({path:info.outputPath('look-narrow-forced.png'),fullPage:true,animations:'disabled'});
    await writeFile(info.outputPath('look-receipt.json'),JSON.stringify({
      scope:'documentary browser application Look switching; not live Body or native proof',
      face:baseline.face, nodes, looks:['conduit.look/phosphor@1','conduit.look/editorial@1','conduit.look/engineering@1'],
      unchanged_face:true, keyboard_selection:true, narrow_forced_colors:true,
      browser:page.context().browser().version(),
    },null,2));
  } finally { lines.close();child.kill('SIGTERM'); }
});
