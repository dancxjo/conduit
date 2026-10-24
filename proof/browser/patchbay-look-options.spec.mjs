import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import { writeFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { expect, test } from '@playwright/test';

// Read actual SVG badge paint, including renderer-supplied inline styles.
async function cordLabelContrasts(page) {
  return page.locator('.flow-cord .react-flow__edge-text').evaluateAll(nodes => {
    const luminance = color => {
      const channels = color.match(/[\d.]+/gu)?.slice(0, 3).map(Number);
      if (channels?.length !== 3) throw new Error(`Unresolved label color: ${color}`);
      const linear = channels.map(channel => { const value=channel/255; return value<=.04045?value/12.92:((value+.055)/1.055)**2.4; });
      return .2126*linear[0]+.7152*linear[1]+.0722*linear[2];
    };
    return nodes.map(node => {
      const background=node.parentElement.querySelector('.react-flow__edge-textbg');
      if (!background) throw new Error('Cord label background absent');
      const ink=getComputedStyle(node).fill, paper=getComputedStyle(background).fill;
      const a=luminance(ink),b=luminance(paper);
      return {text:node.textContent,ink,paper,ratio:(Math.max(a,b)+.05)/(Math.min(a,b)+.05)};
    });
  });
}

// Documentary graph: checks appearance and retained renderer identities, not
// live Body execution or native Mask selection.
test('four typed application Looks retain the exact Face and graph', async ({ page }, info) => {
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
    await expect(page.locator('#lens-label')).toHaveText('PROGRAM · STRUCTURE');
    await expect(page.locator('.flow-frontplate').first()).toBeVisible();
    await page.locator('#fit-flow').click();
    const baseline=await (await page.request.get(`${url}/api/snapshot`)).json();
    const theme=await (await page.request.get(`${url}/assets/theme.css`)).body();
    await writeFile(info.outputPath('theme.css'),theme);
    const cordLabels=[];
    const nodes=await page.locator('.flow-frontplate').evaluateAll(nodes=>nodes.map(node=>node.dataset.subject));
    const picker=page.getByRole('combobox',{name:'Look',exact:true});
    await picker.focus();
    for(const [name,background,font] of [
      ['phosphor','rgb(5, 7, 11)','Georgia'],
      ['editorial','rgb(244, 240, 231)','Georgia'],
      ['engineering','rgb(234, 240, 246)','Inter'],
      ['navy','rgb(16, 27, 42)','Georgia'],
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
      const labels=await cordLabelContrasts(page);expect(labels.length).toBeGreaterThan(0);
      for(const label of labels)expect(label.ratio,`${name} Cord ${label.text}: ${label.ink} / ${label.paper}`).toBeGreaterThanOrEqual(4.5);
      cordLabels.push({look:name,labels});
    }
    await page.setViewportSize({width:390,height:844});
    await page.emulateMedia({forcedColors:'active',reducedMotion:'reduce'});
    await page.addStyleTag({content:':root { font-size:32px !important; }'});
    await expect(page.locator('html')).toHaveCSS('font-size','32px');
    await expect(picker).toHaveCSS('font-size','32px');
    // The narrow action tray scrolls horizontally. Reach the picker through
    // the real Tab sequence so the browser reveals its focused control.
    await page.locator('#arrange').focus();
    await page.keyboard.press('Tab');
    await expect(picker).toBeFocused();
    await page.keyboard.press('Home');
    for(const name of ['phosphor','editorial','engineering','navy']) {
      if(name!=='phosphor')await page.keyboard.press('ArrowDown');
      await expect(picker).toHaveValue(`conduit.look/${name}@1`);
      await picker.focus();
      await expect(picker).toBeFocused();
      expect(await picker.evaluate(node=>node.scrollWidth<=node.clientWidth+1)).toBe(true);
      const box=await picker.boundingBox();
      expect(box.x).toBeGreaterThanOrEqual(0);
      expect(box.x+box.width).toBeLessThanOrEqual(391);
      expect(await picker.evaluate(node=>{
        const control=node.getBoundingClientRect(),tray=node.closest('.canvas-actions').getBoundingClientRect();
        return control.left>=tray.left-.5 && control.right<=tray.right+.5;
      })).toBe(true);
      await expect(page.locator('body')).toHaveAttribute('data-application-look',`conduit.look/${name}@1`);
    }
    expect((await (await page.request.get(`${url}/api/snapshot`)).json()).face).toEqual(baseline.face);
    await page.screenshot({path:info.outputPath('look-narrow-forced.png'),fullPage:true,animations:'disabled'});
    const forcedLabels=await cordLabelContrasts(page);
    for(const label of forcedLabels)expect(label.ratio,'forced-color Cord badge').toBeGreaterThanOrEqual(4.5);
    await writeFile(info.outputPath('look-receipt.json'),JSON.stringify({
      cord_labels:cordLabels,forced_color_cord_labels:forcedLabels,
      scope:'documentary browser application Look switching; not live Body or native proof',
      face:baseline.face, nodes, looks:['conduit.look/phosphor@1','conduit.look/editorial@1','conduit.look/engineering@1','conduit.look/navy@1'],
      unchanged_face:true, keyboard_selection:true, narrow_forced_colors:true,
      browser:page.context().browser().version(),
      theme_css:{bytes:theme.length,sha256:createHash('sha256').update(theme).digest('hex')},
    },null,2));
  } finally { lines.close();child.kill('SIGTERM'); }
});
