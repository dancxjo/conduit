# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: patchbay-panel-furniture.spec.mjs >> enlarged narrow shell retains a useful scrollable graph viewport
- Location: proof/browser/patchbay-panel-furniture.spec.mjs:300:1

# Error details

```
Error: expect(received).toBeGreaterThanOrEqual(expected)

Expected: >= 320
Received:    22
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
            - button "Fit" [active] [ref=e40] [cursor=pointer]
            - button "Center selected" [disabled] [ref=e41]
            - button "Zoom in" [ref=e42] [cursor=pointer]
            - button "Zoom out" [ref=e43] [cursor=pointer]
            - button "Pan right" [ref=e44] [cursor=pointer]
            - button "Arrange" [ref=e45] [cursor=pointer]
            - generic [ref=e46]:
              - text: Look
              - combobox "Look" [ref=e47] [cursor=pointer]:
                - option "Phosphor"
                - option "Editorial"
                - option "Engineering"
                - option "Navy" [selected]
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
  224 |     await page.goto(await server.url);
  225 |     await expect(page.locator("body")).toHaveAttribute("data-application-ready", "true");
  226 |     const before = await page.evaluate(async () => (await (await fetch('/api/snapshot')).json()).face);
  227 |     await page.getByRole("button", { name: "Plot", exact: true }).click();
  228 |     await expect(page.locator("body")).toHaveCSS("font-family", /Georgia/);
  229 |     await expect(page.locator(".flow-frontplate header").first()).toHaveCSS("font-family", /Georgia/);
  230 |     await expect(page.locator(".react-flow__node").filter({ hasText: "text/upper" })).toBeVisible();
  231 |     await page.getByRole("button", { name: "Fit", exact: true }).click();
  232 |     const tokens = await page.locator("body").evaluate(node => {
  233 |       const style = getComputedStyle(node);
  234 |       return ['reading-paper', 'workbench-canvas', 'bootstrap-surface'].map(role => style.getPropertyValue(`--conduit-${role}`).trim());
  235 |     });
  236 |     expect(tokens).toEqual(['#0C121C', '#05070B', '#090D16']);
  237 |     const after = await page.evaluate(async () => (await (await fetch('/api/snapshot')).json()).face);
  238 |     expect(after).toEqual(before);
  239 |     await expect.poll(() => page.evaluate(() => {
  240 |       const graph = document.querySelector('#flow-root').getBoundingClientRect();
  241 |       const heading = document.querySelector('.canvas-heading').getBoundingClientRect();
  242 |       return graph.top >= heading.bottom && graph.height > 0;
  243 |     })).toBe(true);
  244 |     await expect.poll(() => page.locator('#flow-root').evaluate(root => {
  245 |       const bounds = root.getBoundingClientRect();
  246 |       return [...root.querySelectorAll('.react-flow__node')].every(node => {
  247 |         const rect = node.getBoundingClientRect();
  248 |         return rect.top >= bounds.top && rect.bottom <= bounds.bottom && rect.left >= bounds.left && rect.right <= bounds.right;
  249 |       });
  250 |     })).toBe(true);
  251 |     await page.screenshot({ path: testInfo.outputPath('canonical-look-graph.png') });
  252 |   } finally {
  253 |     server.lines.close();
  254 |     server.process.kill("SIGTERM");
  255 |   }
  256 | });
  257 | 
  258 | test("workspace status and tools reserve canvas space instead of covering nodes", async ({ page }, testInfo) => {
  259 |   const server = startServer();
  260 |   try {
  261 |     await page.goto(await server.url);
  262 |     await expect(page.locator("body")).toHaveAttribute("data-application-ready", "true");
  263 |     const clearance = () => page.evaluate(() => {
  264 |       const graph = document.querySelector('#flow-root').getBoundingClientRect();
  265 |       return ['current-status', 'temporal-context', 'front-door-actions', 'workspace-controls', 'flow-notices'].every(id => {
  266 |         const node = document.getElementById(id);
  267 |         if (node.hidden) return true;
  268 |         return node.getBoundingClientRect().top >= graph.bottom;
  269 |       });
  270 |     });
  271 |     await expect.poll(clearance).toBe(true);
  272 |     await page.locator('#workspace-controls > summary').click();
  273 |     await expect(page.getByRole('textbox', { name: 'Workspace document', exact: true })).toBeAttached();
  274 |     await expect.poll(clearance).toBe(true);
  275 |     await page.getByRole('textbox', { name: 'Layout name', exact: true }).focus();
  276 |     await expect(page.getByRole('textbox', { name: 'Layout name', exact: true })).toBeInViewport();
  277 |     await page.locator('#workspace-controls > summary').click();
  278 |     await page.getByRole('button', { name: 'Plot', exact: true }).click();
  279 |     await expect(page.locator('.react-flow__node').filter({ hasText: 'text/upper' })).toBeVisible();
  280 |     await page.getByRole('button', { name: 'Fit', exact: true }).click();
  281 |     await expect(page.locator('#flow-notices')).toContainText('omitted from diagram. Exact Face facts remain available.');
  282 |     await page.screenshot({ path: testInfo.outputPath('reserved-status-wide.png') });
  283 |     await page.setViewportSize({ width: 390, height: 1000 });
  284 |     await page.addStyleTag({ content: 'html{font-size:32px!important}' });
  285 |     await page.emulateMedia({ forcedColors: 'active', reducedMotion: 'reduce' });
  286 |     await expect.poll(clearance).toBe(true);
  287 |     await expect(page.locator('#current-status')).toBeVisible();
  288 |     expect(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth)).toBe(false);
  289 |     await page.locator('#workspace-controls > summary').click();
  290 |     await page.getByRole('textbox', { name: 'Layout name', exact: true }).focus();
  291 |     await expect(page.getByRole('textbox', { name: 'Layout name', exact: true })).toBeInViewport();
  292 |     await page.screenshot({ path: testInfo.outputPath('reserved-status-narrow.png') });
  293 |   } finally {
  294 |     server.lines.close();
  295 |     server.process.kill('SIGTERM');
  296 |   }
  297 | });
  298 | 
  299 | 
  300 | test("enlarged narrow shell retains a useful scrollable graph viewport", async ({ page }, info) => {
  301 |   const server=startServer();
  302 |   try {
  303 |     await page.setViewportSize({width:390,height:844});
  304 |     await page.goto(await server.url);
  305 |     await expect(page.locator('body')).toHaveAttribute('data-application-ready','true');
  306 |     await page.getByRole('button',{name:'Plot',exact:true}).click();
  307 |     await page.getByRole('combobox',{name:'Look',exact:true}).selectOption('conduit.look/navy@1');
  308 |     await page.addStyleTag({content:'html{font-size:32px!important}'});
  309 |     await page.emulateMedia({forcedColors:'active',reducedMotion:'reduce'});
  310 |     await expect(page.locator('html')).toHaveCSS('font-size','32px');
  311 |     await page.getByRole('button',{name:'Fit',exact:true}).click();
  312 |     const baseline=await (await page.request.get(`${await server.url}/api/snapshot`)).json();
  313 |     const graph=page.locator('#flow-root');
  314 |     await graph.scrollIntoViewIfNeeded();
  315 |     const geometry=await graph.evaluate(node=>{
  316 |       const r=node.getBoundingClientRect(),top=Math.max(0,r.top),bottom=Math.min(innerHeight,r.bottom);
  317 |       return {viewport:{width:innerWidth,height:innerHeight},graph:{top:r.top,bottom:r.bottom,width:r.width,height:r.height},
  318 |         visibleHeight:Math.max(0,bottom-top),documentHeight:document.documentElement.scrollHeight,
  319 |         horizontalOverflow:document.documentElement.scrollWidth>innerWidth,
  320 |         graphHitSamples:[.2,.5,.8].map(fraction=>node.contains(document.elementFromPoint(r.left+r.width/2,top+(bottom-top)*fraction)))};
  321 |     });
  322 |     await writeFile(info.outputPath('narrow-graph-geometry.json'),JSON.stringify(geometry,null,2));
  323 |     await page.screenshot({path:info.outputPath('narrow-graph.png'),fullPage:true,animations:'disabled'});
> 324 |     expect(geometry.graph.height).toBeGreaterThanOrEqual(320);
      |                                   ^ Error: expect(received).toBeGreaterThanOrEqual(expected)
  325 |     expect(geometry.visibleHeight).toBeGreaterThanOrEqual(320);
  326 |     expect(geometry.documentHeight).toBeGreaterThan(geometry.viewport.height);
  327 |     expect(geometry.horizontalOverflow).toBe(false);
  328 |     expect(geometry.graphHitSamples).toEqual([true,true,true]);
  329 |     const tools=page.locator('#workspace-controls > summary');
  330 |     await tools.focus();await expect(tools).toBeFocused();await expect(tools).toBeInViewport();
  331 |     const current=await (await page.request.get(`${await server.url}/api/snapshot`)).json();
  332 |     expect(current.face).toEqual(baseline.face);
  333 |     await graph.scrollIntoViewIfNeeded();
  334 |     await page.screenshot({path:info.outputPath('narrow-graph-returned.png'),fullPage:true,animations:'disabled'});
  335 |   } finally {server.lines.close();server.process.kill('SIGTERM');}
  336 | });
  337 | 
```