# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: patchbay-look-options.spec.mjs >> four typed application Looks retain the exact Face and graph
- Location: proof/browser/patchbay-look-options.spec.mjs:28:1

# Error details

```
Error: phosphor Cord value/text: rgb(147, 210, 247) / rgb(255, 255, 255)

expect(received).toBeGreaterThanOrEqual(expected)

Expected: >= 4.5
Received:    1.6403268893714602
```

# Page snapshot

```yaml
- generic [ref=e1]:
  - link "Skip to main content" [ref=e2] [cursor=pointer]:
    - /url: "#main-content"
  - generic [ref=e3]:
    - banner [ref=e5]:
      - navigation "Conduit products" [ref=e6]:
        - link "conduit" [ref=e7] [cursor=pointer]:
          - /url: /conduit
        - link "Patchbay" [ref=e8] [cursor=pointer]:
          - /url: /conduit/patchbay/
        - link "Source" [ref=e9] [cursor=pointer]:
          - /url: https://github.com/dancxjo/conduit
      - status [ref=e10]: Face revision 1 · content 0e0a449d8c1b4f0d837a3239b7cc37ecfcba98f8cd2b9f2abdf24b60ec4621ff · Manifestation Available · read-only
    - navigation "Patchbay workspace" [ref=e11]:
      - generic "Patchbay surfaces" [ref=e12]:
        - button "Library" [ref=e13] [cursor=pointer]
        - button "Inspect" [ref=e14] [cursor=pointer]
        - button "Body overview" [ref=e15] [cursor=pointer]
    - main [ref=e19]:
      - heading "Conduit Patchbay" [level=1] [ref=e20]
      - region [ref=e22]:
        - generic [ref=e23]:
          - generic [ref=e24]:
            - paragraph [ref=e25]: PROGRAM · STRUCTURE
            - heading "Program structure" [level=2] [ref=e26]
            - generic [ref=e27]:
              - navigation "Available Places" [ref=e29]:
                - button "Back" [ref=e30] [cursor=pointer]
                - button "Body" [ref=e31] [cursor=pointer]
                - button "Plot" [ref=e32] [cursor=pointer]
              - navigation "Available views" [ref=e34]:
                - button "Structure" [ref=e35] [cursor=pointer]
                - button "Realization" [ref=e36] [cursor=pointer]
                - button "Play" [ref=e37] [cursor=pointer]
                - button "Debug" [ref=e38] [cursor=pointer]
          - generic "Canvas actions" [ref=e39]:
            - button "Fit" [ref=e40] [cursor=pointer]
            - button "Center selected" [disabled] [ref=e41]
            - button "Zoom in" [ref=e42] [cursor=pointer]
            - button "Zoom out" [ref=e43] [cursor=pointer]
            - button "Pan right" [ref=e44] [cursor=pointer]
            - button "Arrange" [ref=e45] [cursor=pointer]
            - generic [ref=e46]:
              - text: Look
              - combobox "Look" [active] [ref=e47] [cursor=pointer]:
                - option "Phosphor" [selected]
                - option "Editorial"
                - option "Engineering"
                - option "Navy"
            - button "High contrast" [ref=e48] [cursor=pointer]
        - region "Patchbay spatial workspace" [ref=e49]:
          - generic [ref=e50]:
            - generic [ref=e51]:
              - generic:
                - img:
                  - generic:
                    - generic [ref=e52]: value/text
                    - generic [ref=e55]: value/text
                - generic:
                  - article "Current checked and expanded Plot" [ref=e60]:
                    - generic [ref=e61]:
                      - radio "Select Current checked and expanded Plot" [ref=e62] [cursor=pointer]
                      - generic "Current checked and expanded Plot" [ref=e64]
                      - generic "Plot" [ref=e65]
                    - paragraph [ref=e66]: checked intent
                  - article "hello/literal-6f8beb2a9ae7-0 Gear, text/literal" [ref=e68]:
                    - generic [ref=e69]:
                      - radio "Select hello/literal-6f8beb2a9ae7-0 Gear, text/literal" [ref=e70] [cursor=pointer]
                      - generic "hello/literal-6f8beb2a9ae7-0 Gear, text/literal" [ref=e72]: literal
                      - generic "Gear" [ref=e73]
                    - paragraph [ref=e74]: text/literal
                    - group "Exact typed Ports" [ref=e75]:
                      - generic [ref=e76]:
                        - button "Select text Output Port carrying value/text" [ref=e77] [cursor=pointer]: text Output Port carrying value/text
                        - code [ref=e79]: value/text
                        - img "text Output Port carrying value/text" [ref=e80]
                  - article "hello/show Gear, face/text" [ref=e82]:
                    - generic [ref=e83]:
                      - radio "Select hello/show Gear, face/text" [ref=e84] [cursor=pointer]
                      - generic "hello/show Gear, face/text" [ref=e86]: text
                      - generic "Gear" [ref=e87]
                    - paragraph [ref=e88]: face/text
                    - group "Exact typed Ports" [ref=e89]:
                      - generic [ref=e90]:
                        - img "text Input Port carrying value/text" [ref=e91]
                        - button "Select text Input Port carrying value/text" [ref=e92] [cursor=pointer]: text Input Port carrying value/text
                        - code [ref=e94]: value/text
                  - article "hello/upper Gear, text/upper" [ref=e96]:
                    - generic [ref=e97]:
                      - radio "Select hello/upper Gear, text/upper" [ref=e98] [cursor=pointer]
                      - generic "hello/upper Gear, text/upper" [ref=e100]: upper
                      - generic "Gear" [ref=e101]
                    - paragraph [ref=e102]: text/upper
                    - group "Exact typed Ports" [ref=e103]:
                      - generic [ref=e104]:
                        - button "Select text Output Port carrying value/text" [ref=e105] [cursor=pointer]: text Output Port carrying value/text
                        - code [ref=e107]: value/text
                        - img "text Output Port carrying value/text" [ref=e108]
                      - generic [ref=e109]:
                        - img "source Input Port carrying value/text" [ref=e110]
                        - button "Select source Input Port carrying value/text" [ref=e111] [cursor=pointer]: source Input Port carrying value/text
                        - code [ref=e113]: value/text
                  - article "Plot hello checked 900bb4c38ea032d7495d7d1a66f1a57b683846c64751c7beb7972509852a0b80" [ref=e115]:
                    - generic [ref=e116]:
                      - radio "Select Plot hello checked 900bb4c38ea032d7495d7d1a66f1a57b683846c64751c7beb7972509852a0b80" [ref=e117] [cursor=pointer]
                      - generic "Plot hello checked 900bb4c38ea032d7495d7d1a66f1a57b683846c64751c7beb7972509852a0b80" [ref=e119]
                      - generic "Plot" [ref=e120]
                    - paragraph [ref=e121]: checked intent
            - generic [ref=e125]:
              - button "zoom in" [ref=e126] [cursor=pointer]
              - button "zoom out" [ref=e129] [cursor=pointer]
              - button "fit view" [ref=e132] [cursor=pointer]
            - link "React Flow" [ref=e136] [cursor=pointer]:
              - /url: https://reactflow.dev
          - paragraph [ref=e137]
      - region "Workspace status and tools" [ref=e138]:
        - group [ref=e139]:
          - generic "Workspace layouts and annotations (face only)" [ref=e140] [cursor=pointer]
          - option "Default" [selected]
          - option "straight" [selected]
          - option "curved"
          - option "orthogonal"
          - option "manual"
        - region "Current run and diagnostic status" [ref=e141]:
          - strong [ref=e142]: Current truth
          - generic [ref=e143]: Manifestation Available · Plan ready · Play active
          - generic [ref=e144]: No checked diagnostics
          - generic [ref=e145]:
            - status: 2/5 projected Cords drawn · 3 omitted from diagram. Exact Face facts remain available.
        - region [ref=e146]:
          - strong [ref=e147]: Current observations
          - article [ref=e152]:
            - heading "between 4 and 6 seconds ago · observation" [level=3] [ref=e153]
            - group [ref=e154]:
              - generic "Exact time and clock provenance" [ref=e155]
        - region "Current plot realization" [ref=e156]:
          - strong [ref=e157]: Run status
          - group "Available actions" [ref=e159]:
            - button "Plan current plot" [disabled] [ref=e160]
            - button "Play current plan" [disabled] [ref=e161]
          - status [ref=e163]: World truth is current; planning is explicit.
```

# Test source

```ts
  1   | import { spawn } from 'node:child_process';
  2   | import { createInterface } from 'node:readline';
  3   | import { writeFile } from 'node:fs/promises';
  4   | import { createHash } from 'node:crypto';
  5   | import { expect, test } from '@playwright/test';
  6   | 
  7   | // Read actual SVG badge paint, including renderer-supplied inline styles.
  8   | async function cordLabelContrasts(page) {
  9   |   return page.locator('.flow-cord .react-flow__edge-text').evaluateAll(nodes => {
  10  |     const luminance = color => {
  11  |       const channels = color.match(/[\d.]+/gu)?.slice(0, 3).map(Number);
  12  |       if (channels?.length !== 3) throw new Error(`Unresolved label color: ${color}`);
  13  |       const linear = channels.map(channel => { const value=channel/255; return value<=.04045?value/12.92:((value+.055)/1.055)**2.4; });
  14  |       return .2126*linear[0]+.7152*linear[1]+.0722*linear[2];
  15  |     };
  16  |     return nodes.map(node => {
  17  |       const background=node.parentElement.querySelector('.react-flow__edge-textbg');
  18  |       if (!background) throw new Error('Cord label background absent');
  19  |       const ink=getComputedStyle(node).fill, paper=getComputedStyle(background).fill;
  20  |       const a=luminance(ink),b=luminance(paper);
  21  |       return {text:node.textContent,ink,paper,ratio:(Math.max(a,b)+.05)/(Math.min(a,b)+.05)};
  22  |     });
  23  |   });
  24  | }
  25  | 
  26  | // Documentary graph: checks appearance and retained renderer identities, not
  27  | // live Body execution or native Mask selection.
  28  | test('four typed application Looks retain the exact Face and graph', async ({ page }, info) => {
  29  |   const child=spawn('target/debug/conduit-browser-patchbay-workbench',['--documentary-fixture'],{stdio:['ignore','pipe','pipe']});
  30  |   const errors=[];child.stderr.on('data',chunk=>errors.push(String(chunk)));
  31  |   const lines=createInterface({input:child.stdout});
  32  |   const url=await new Promise((resolve,reject)=>{
  33  |     lines.once('line',line=>resolve(line.replace('PATCHBAY_HTML_URL=','')));
  34  |     child.once('exit',code=>reject(new Error(`server ${code}: ${errors.join('')}`)));
  35  |   });
  36  |   try {
  37  |     await page.setViewportSize({width:1440,height:1000});
  38  |     await page.goto(url);
  39  |     await expect(page.locator('body')).toHaveAttribute('data-application-ready','true');
  40  |     await page.getByRole('button',{name:'Plot',exact:true}).click();
  41  |     await expect(page.locator('#lens-label')).toHaveText('PROGRAM · STRUCTURE');
  42  |     await expect(page.locator('.flow-frontplate').first()).toBeVisible();
  43  |     await page.locator('#fit-flow').click();
  44  |     const baseline=await (await page.request.get(`${url}/api/snapshot`)).json();
  45  |     const theme=await (await page.request.get(`${url}/assets/theme.css`)).body();
  46  |     await writeFile(info.outputPath('theme.css'),theme);
  47  |     const cordLabels=[];
  48  |     const nodes=await page.locator('.flow-frontplate').evaluateAll(nodes=>nodes.map(node=>node.dataset.subject));
  49  |     const picker=page.getByRole('combobox',{name:'Look',exact:true});
  50  |     await picker.focus();
  51  |     for(const [name,background,font] of [
  52  |       ['phosphor','rgb(5, 7, 11)','Georgia'],
  53  |       ['editorial','rgb(244, 240, 231)','Georgia'],
  54  |       ['engineering','rgb(234, 240, 246)','Inter'],
  55  |       ['navy','rgb(16, 27, 42)','Georgia'],
  56  |     ]) {
  57  |       if(name!=='phosphor')await page.keyboard.press('ArrowDown');
  58  |       await expect(picker).toHaveValue(`conduit.look/${name}@1`);
  59  |       await expect(picker).toBeFocused();
  60  |       await expect(page.locator('body')).toHaveCSS('background-color',background);
  61  |       const bodyFont=await page.locator('body').evaluate(node=>getComputedStyle(node).fontFamily);
  62  |       expect(bodyFont).toContain(font);
  63  |       expect(await page.locator('.flow-frontplate').evaluateAll(nodes=>nodes.map(node=>node.dataset.subject))).toEqual(nodes);
  64  |       const current=await (await page.request.get(`${url}/api/snapshot`)).json();
  65  |       expect(current.face).toEqual(baseline.face);
  66  |       await page.screenshot({path:info.outputPath(`look-${name}.png`),fullPage:true,animations:'disabled'});
  67  |       const labels=await cordLabelContrasts(page);expect(labels.length).toBeGreaterThan(0);
> 68  |       for(const label of labels)expect(label.ratio,`${name} Cord ${label.text}: ${label.ink} / ${label.paper}`).toBeGreaterThanOrEqual(4.5);
      |                                                                                                                 ^ Error: phosphor Cord value/text: rgb(147, 210, 247) / rgb(255, 255, 255)
  69  |       cordLabels.push({look:name,labels});
  70  |     }
  71  |     await page.setViewportSize({width:390,height:844});
  72  |     await page.emulateMedia({forcedColors:'active',reducedMotion:'reduce'});
  73  |     await page.addStyleTag({content:':root { font-size:32px !important; }'});
  74  |     await expect(page.locator('html')).toHaveCSS('font-size','32px');
  75  |     await expect(picker).toHaveCSS('font-size','32px');
  76  |     // The narrow action tray scrolls horizontally. Reach the picker through
  77  |     // the real Tab sequence so the browser reveals its focused control.
  78  |     await page.locator('#arrange').focus();
  79  |     await page.keyboard.press('Tab');
  80  |     await expect(picker).toBeFocused();
  81  |     await page.keyboard.press('Home');
  82  |     for(const name of ['phosphor','editorial','engineering','navy']) {
  83  |       if(name!=='phosphor')await page.keyboard.press('ArrowDown');
  84  |       await expect(picker).toHaveValue(`conduit.look/${name}@1`);
  85  |       await picker.focus();
  86  |       await expect(picker).toBeFocused();
  87  |       expect(await picker.evaluate(node=>node.scrollWidth<=node.clientWidth+1)).toBe(true);
  88  |       const box=await picker.boundingBox();
  89  |       expect(box.x).toBeGreaterThanOrEqual(0);
  90  |       expect(box.x+box.width).toBeLessThanOrEqual(391);
  91  |       expect(await picker.evaluate(node=>{
  92  |         const control=node.getBoundingClientRect(),tray=node.closest('.canvas-actions').getBoundingClientRect();
  93  |         return control.left>=tray.left-.5 && control.right<=tray.right+.5;
  94  |       })).toBe(true);
  95  |       await expect(page.locator('body')).toHaveAttribute('data-application-look',`conduit.look/${name}@1`);
  96  |     }
  97  |     expect((await (await page.request.get(`${url}/api/snapshot`)).json()).face).toEqual(baseline.face);
  98  |     await page.screenshot({path:info.outputPath('look-narrow-forced.png'),fullPage:true,animations:'disabled'});
  99  |     const forcedLabels=await cordLabelContrasts(page);
  100 |     for(const label of forcedLabels)expect(label.ratio,'forced-color Cord badge').toBeGreaterThanOrEqual(4.5);
  101 |     await writeFile(info.outputPath('look-receipt.json'),JSON.stringify({
  102 |       cord_labels:cordLabels,forced_color_cord_labels:forcedLabels,
  103 |       scope:'documentary browser application Look switching; not live Body or native proof',
  104 |       face:baseline.face, nodes, looks:['conduit.look/phosphor@1','conduit.look/editorial@1','conduit.look/engineering@1','conduit.look/navy@1'],
  105 |       unchanged_face:true, keyboard_selection:true, narrow_forced_colors:true,
  106 |       browser:page.context().browser().version(),
  107 |       theme_css:{bytes:theme.length,sha256:createHash('sha256').update(theme).digest('hex')},
  108 |     },null,2));
  109 |   } finally { lines.close();child.kill('SIGTERM'); }
  110 | });
  111 | 
```