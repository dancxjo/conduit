# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: patchbay-front-door.spec.mjs >> public browser entrance stays unbodied until OPEN then explicit BIRTH
- Location: proof/browser/patchbay-front-door.spec.mjs:109:1

# Error details

```
TypeError: Cannot read properties of undefined (reading 'identity')
```

# Page snapshot

```yaml
- generic [active] [ref=e1]:
  - link "Skip to main content" [ref=e2] [cursor=pointer]:
    - /url: "#main-content"
  - status:
    - generic:
      - strong: Conduit Patchbay
      - generic: Preparing the workbench…
  - generic [ref=e3]:
    - banner [ref=e5]:
      - navigation "Conduit products" [ref=e6]:
        - link "Conduit home" [ref=e7] [cursor=pointer]:
          - /url: /conduit
          - text: conduit
        - link "Patchbay" [ref=e8] [cursor=pointer]:
          - /url: /conduit/patchbay/
        - link "Source" [ref=e9] [cursor=pointer]:
          - /url: https://github.com/dancxjo/conduit
      - status [ref=e10]: Presentation revision 4 · content 5f974e9653546f8fd85a3bfeb63b0cc29eed36ef63719fef046e9f62307e7366 · Manifestation Available · canonical authoring
    - navigation "Patchbay workspace" [ref=e11]:
      - generic "Patchbay surfaces" [ref=e12]:
        - button "Library" [expanded] [ref=e13] [cursor=pointer]
        - button "Inspect" [expanded] [ref=e14] [cursor=pointer]
        - button "No Body" [ref=e15] [cursor=pointer]
    - main [ref=e16]:
      - heading "Conduit Patchbay" [level=1] [ref=e17]
      - generic [ref=e18]:
        - complementary [ref=e19]:
          - group "Library furniture" [ref=e21]:
            - paragraph [ref=e22]: Library
            - button "Collapse Library" [ref=e23] [cursor=pointer]
            - button "Move Library to right" [ref=e24] [cursor=pointer]
            - button "Close Library" [ref=e25] [cursor=pointer]
          - heading "Library" [level=2] [ref=e26]
          - text: Find forms, gears, and parts
          - searchbox "Find forms, gears, and parts" [ref=e27]: hElLo
          - region [ref=e28]:
            - heading "gears · reusable kinds" [level=3] [ref=e29]
            - paragraph [ref=e30]: Place semantic kinds into the opened canonical form.
            - status [ref=e32]: 0 of 78 Gears available from the canonical catalog
            - generic "Available gear kinds"
          - group [ref=e33]:
            - generic "Exact form identity" [ref=e34]
        - region [ref=e35]:
          - generic [ref=e37]:
            - paragraph [ref=e38]: PROGRAM · STRUCTURE
            - heading "Program structure" [level=2] [ref=e39]
            - generic [ref=e40]:
              - navigation "Available Places" [ref=e42]:
                - button "Back" [ref=e43] [cursor=pointer]
                - button "Entrance" [ref=e44] [cursor=pointer]
                - button "Program" [ref=e45] [cursor=pointer]
              - navigation "Available views" [ref=e47]:
                - button "Structure" [ref=e48] [cursor=pointer]
                - button "Debug" [ref=e49] [cursor=pointer]
          - generic "Patchbay spatial workspace" [ref=e50]:
            - generic [ref=e51]:
              - generic [ref=e52]:
                - generic:
                  - img:
                    - generic:
                      - generic [ref=e53]: value/text
                      - generic [ref=e57]: value/text
                  - generic:
                    - article "Hello" [ref=e61]:
                      - generic [ref=e62]:
                        - radio "Select Hello" [checked] [ref=e63] [cursor=pointer]
                        - generic [ref=e64]: ◇
                        - generic "Hello" [ref=e65]
                        - generic "Form" [ref=e66]
                      - paragraph [ref=e67]: checked intent
                    - article "hello/literal-6f8beb2a9ae7-0 Gear, text/literal" [ref=e69]:
                      - generic [ref=e70]:
                        - radio "Select hello/literal-6f8beb2a9ae7-0 Gear, text/literal" [ref=e71] [cursor=pointer]
                        - generic [ref=e72]: type
                        - generic "hello/literal-6f8beb2a9ae7-0 Gear, text/literal" [ref=e73]: literal
                        - generic "Gear" [ref=e74]
                      - paragraph [ref=e75]: text/literal
                      - group "Exact typed Ports" [ref=e76]:
                        - generic [ref=e77]:
                          - generic "text Output Port carrying value/text"
                          - code [ref=e78]: value/text
                          - img "text Output Port carrying value/text" [ref=e79]
                    - article "hello/show Gear, presentation/text" [ref=e81]:
                      - generic [ref=e82]:
                        - radio "Select hello/show Gear, presentation/text" [ref=e83] [cursor=pointer]
                        - generic [ref=e84]: presentation
                        - generic "hello/show Gear, presentation/text" [ref=e85]: text
                        - generic "Gear" [ref=e86]
                      - paragraph [ref=e87]: presentation/text
                      - group "Exact typed Ports" [ref=e88]:
                        - generic [ref=e89]:
                          - img "text Input Port carrying value/text" [ref=e90]
                          - generic "text Input Port carrying value/text"
                          - code [ref=e91]: value/text
                    - article "hello/upper Gear, text/upper" [ref=e93]:
                      - generic [ref=e94]:
                        - radio "Select hello/upper Gear, text/upper" [ref=e95] [cursor=pointer]
                        - generic [ref=e96]: case-upper
                        - generic "hello/upper Gear, text/upper" [ref=e97]: upper
                        - generic "Gear" [ref=e98]
                      - paragraph [ref=e99]: text/upper
                      - group "Exact typed Ports" [ref=e100]:
                        - generic [ref=e101]:
                          - generic "text Output Port carrying value/text"
                          - code [ref=e102]: value/text
                          - img "text Output Port carrying value/text" [ref=e103]
                        - generic [ref=e104]:
                          - img "text Input Port carrying value/text" [ref=e105]
                          - generic "text Input Port carrying value/text"
                          - code [ref=e106]: value/text
              - generic [ref=e110]:
                - button "zoom in" [ref=e111] [cursor=pointer]
                - button "zoom out" [ref=e114] [cursor=pointer]
                - button "fit view" [ref=e117] [cursor=pointer]
              - link "React Flow" [ref=e121] [cursor=pointer]:
                - /url: https://reactflow.dev
            - paragraph [ref=e122]
        - complementary [ref=e123]:
          - group "Inspect furniture" [ref=e125]:
            - paragraph [ref=e126]: Inspect
            - button "Collapse Inspect" [ref=e127] [cursor=pointer]
            - button "Move Inspect to bottom" [ref=e128] [cursor=pointer]
            - button "Close Inspect" [ref=e129] [cursor=pointer]
          - heading "Inspect" [level=2] [ref=e130]
          - button "Clear selection" [ref=e131] [cursor=pointer]
          - paragraph [ref=e132]: Hello
          - generic "Exact facts" [ref=e134]:
            - generic [ref=e135]:
              - term [ref=e136]: Meaning
              - definition [ref=e137]: Hello
            - generic [ref=e138]:
              - term [ref=e139]: Subject
              - definition [ref=e140]: Form
            - generic [ref=e141]:
              - term [ref=e142]: freshness-sequence
              - definition [ref=e143]: "3"
            - generic [ref=e144]:
              - term [ref=e145]: opened
              - definition [ref=e146]: "true"
          - group "Selected subject actions" [ref=e148]:
            - button "OPEN" [ref=e149] [cursor=pointer]
            - status [ref=e150]: Available
            - button "BIRTH" [ref=e151] [cursor=pointer]
            - status [ref=e152]: Available
            - button "SAVE" [ref=e153] [cursor=pointer]
            - status [ref=e154]: Available
          - group [ref=e155]:
            - generic "Related subjects" [ref=e156]
          - group [ref=e157]:
            - generic "Exact details" [ref=e158]
          - group [ref=e159]:
            - generic "Advanced evidence and linear presentation" [ref=e160]
      - region "Current run and diagnostic status" [ref=e161]:
        - strong [ref=e162]: Current truth
        - generic [ref=e163]: Manifestation Available · not planned · not playing
        - generic [ref=e164]: No checked diagnostics
      - region [ref=e165]:
        - strong [ref=e166]: Current observations
      - region "Current form realization" [ref=e167]:
        - strong [ref=e168]: Run status
        - group "Available actions" [ref=e170]:
          - button "Plan current form" [disabled] [ref=e171]
          - button "Play current plan" [disabled] [ref=e172]
        - status [ref=e174]: World truth is current; planning is explicit.
```

# Test source

```ts
  86  | }
  87  | 
  88  | async function refuseInvocation(page, refusals, operation, request, expected) {
  89  |   const before = await snapshot(page);
  90  |   const after = await page.evaluate(async body => (await fetch("/api/interaction", {
  91  |     method: "POST",
  92  |     headers: { "Content-Type": "application/json" },
  93  |     body: JSON.stringify(body),
  94  |   })).json(), request);
  95  |   expect(after.interaction.last_disposition).toBe(`Refused(${expected})`);
  96  |   expect(after.navigation.cursor).toEqual(before.navigation.cursor);
  97  |   expect(after.presentation.identity).toBe(before.presentation.identity);
  98  |   expect(after.presentation.revision).toBe(before.presentation.revision);
  99  |   expect(semanticBasis(after)).toEqual(semanticBasis(before));
  100 |   refusals.push({
  101 |     operation,
  102 |     disposition: after.interaction.last_disposition,
  103 |     cursor: after.navigation.cursor,
  104 |     semantic_basis: semanticBasis(after),
  105 |   });
  106 |   return after;
  107 | }
  108 | 
  109 | test("public browser entrance stays unbodied until OPEN then explicit BIRTH", async ({ browser, page }) => {
  110 |   const server = startPublicEntrance();
  111 |   try {
  112 |     const url = await server.url;
  113 |     const initial = await (await fetch(`${url}/api/snapshot`)).json();
  114 |     expect(initial.entrance.layer).toBe("World");
  115 |     expect(initial.entrance.body_id).toBeNull();
  116 |     expect(initial.entrance.selected_subject).toMatch(/^host\//);
  117 |     expect(initial.parts).toBeUndefined();
  118 |     expect(initial.presentation.basis).toMatchObject({
  119 |       body_id: null,
  120 |       wake_id: null,
  121 |       source_document_id: null,
  122 |       checked_form_id: null,
  123 |       expanded_form_id: null,
  124 |       plan_id: null,
  125 |       active_play_id: null,
  126 |     });
  127 |     expect(initial.presentation.subjects.some(({ role }) => role === "Host")).toBe(true);
  128 |     expect(initial.presentation.subjects.some(({ role }) => role === "Form")).toBe(true);
  129 |     expect(initial.presentation.subjects.some(({ role }) => role === "Body")).toBe(false);
  130 |     expect(initial.presentation.subjects.some(({ role }) => role === "Part")).toBe(false);
  131 |     expect(initial.presentation.properties).toEqual(expect.arrayContaining([
  132 |       expect.objectContaining({ name: "current-body", value: { Text: "none" } }),
  133 |       expect.objectContaining({ name: "this-host", value: { Flag: true } }),
  134 |     ]));
  135 | 
  136 |     await page.goto(url);
  137 |     const navigationRefusals = [];
  138 |     await refuseNavigation(page, navigationRefusals, { kind: "show", aspect: "Plan" }, {
  139 |       presentation_id: initial.presentation.identity,
  140 |       presentation_revision: initial.presentation.revision,
  141 |       navigation_id: initial.navigation.navigation.identity,
  142 |       operation: { kind: "show", aspect: "Plan" },
  143 |     }, "UnknownAspect");
  144 |     await expect(page.getByRole("heading", { name: "Entrance choices" })).toBeVisible();
  145 |     await expect(page.locator("body")).toHaveAttribute("data-place", "Entrance");
  146 |     await expect(page.locator('[data-application-key="product-status"]')).toContainText("Manifestation Available");
  147 |     expect(await page.evaluate(() => globalThis.__patchbayMembership)).toBeUndefined();
  148 |     expect(await page.locator('#subjects input[type="radio"]').evaluateAll((choices) =>
  149 |       choices.every((choice) => !["Body", "Part", "Gear", "Port", "Cord", "Line"].includes(choice.dataset.role)),
  150 |     )).toBe(true);
  151 |     const workspaceBox = await page.locator(".workspace").boundingBox();
  152 |     expect(workspaceBox.y + workspaceBox.height).toBeLessThanOrEqual(768);
  153 |     await expect(page.getByRole("button", { name: "Library", exact: true })).toHaveAttribute("aria-expanded", "true");
  154 |     await expect(page.locator("#form-results").getByRole("button")).toHaveCount(3);
  155 |     await expect(page.getByRole("button", { name: "Open Form Text Lab" })).toBeVisible();
  156 |     await page.getByRole("searchbox", { name: "Find Forms, Gears, and Parts" }).fill("hElLo");
  157 |     await expect(page.locator("#form-results-status")).toHaveText("1 of 3 Forms available");
  158 |     const form = initial.presentation.subjects.find(({ role, label }) => role === "Form" && label === "Hello");
  159 |     const formButton = page.getByRole("button", { name: "Open Form Hello" });
  160 |     await page.getByRole("searchbox", { name: "Find Forms, Gears, and Parts" }).press("ArrowDown");
  161 |     await expect(formButton).toBeFocused();
  162 |     await page.evaluate(() => window.patchbayReload());
  163 |     await expect(formButton).toBeFocused();
  164 |     const openResponses = [];
  165 |     let resolveOpenSequence;
  166 |     const openSequence = new Promise((resolve) => { resolveOpenSequence = resolve; });
  167 |     page.on("response", (response) => {
  168 |       if(["/api/navigation", "/api/interaction"].some(path => response.url().endsWith(path)) && response.request().method() === "POST") {
  169 |         openResponses.push(response);
  170 |         if(openResponses.length === 2) resolveOpenSequence();
  171 |       }
  172 |     });
  173 |     await Promise.all([openSequence, formButton.press("Enter")]);
  174 |     expect(openResponses).toHaveLength(2);
  175 |     expect(openResponses.map(response => new URL(response.url()).pathname).sort()).toEqual([
  176 |       "/api/interaction",
  177 |       "/api/navigation",
  178 |     ]);
  179 |     await expect(page.locator("body")).toHaveAttribute("data-inspector-open", "true");
  180 |     const birthButton = page.getByRole("button", { name: "BIRTH" });
  181 |     await expect(birthButton).toBeVisible();
  182 |     await expect(birthButton).toBeEnabled();
  183 |     expect(openResponses.every((response) => response.ok())).toBe(true);
  184 |     await expect(page.getByRole("button", { name: "OPEN", exact: true })).toBeEnabled();
  185 |     const openAction = initial.presentation.actions.find(
> 186 |       ({ intent, target }) => intent === "conduit.intent/open@1" && target === form.identity,
      |                                                                                     ^ TypeError: Cannot read properties of undefined (reading 'identity')
  187 |     );
  188 |     const opened = await (await fetch(`${url}/api/snapshot`)).json();
  189 |     expect(opened.interaction.last_request_id).toMatch(/^navigation\//);
  190 |     expect(opened.interaction.last_disposition).toBe("Succeeded");
  191 |     expect(openAction.identity).toMatch(/^action\/open\//);
  192 |     expect(opened.revision).toBe(initial.revision + 1);
  193 |     expect(opened.presentation.basis.body_id).toBeNull();
  194 |     expect(opened.navigation.cursor.place).toBe("Program");
  195 |     expect(opened.parts).toBeUndefined();
  196 |     expect(opened.presentation.properties).toContainEqual(
  197 |       expect.objectContaining({ subject: form.identity, name: "opened", value: { Flag: true } }),
  198 |     );
  199 |     expect(opened.presentation.subjects).toEqual(expect.arrayContaining([
  200 |       expect.objectContaining({ role: "Form" }),
  201 |       expect.objectContaining({ role: "Gear", label: "hello/upper" }),
  202 |       expect.objectContaining({ role: "Cord" }),
  203 |     ]));
  204 |     const upperFaceplate = page.locator('.faceplate-title[title="hello/upper"]');
  205 |     await expect(upperFaceplate).toHaveText("upper");
  206 |     await expect(upperFaceplate).toHaveAttribute("title", "hello/upper");
  207 |     const birthAction = opened.presentation.actions.find(
  208 |       ({ intent, target }) => intent === "conduit.intent/birth@1" && target === form.identity,
  209 |     );
  210 |     expect(birthAction.identity).toMatch(/^action\/birth\//);
  211 | 
  212 |     const exact = page.locator("#inspector .exact-selection");
  213 |     await expect(exact).not.toHaveAttribute("open", "");
  214 |     await exact.locator("summary").click();
  215 |     await expect(exact).toHaveAttribute("open", "");
  216 |     await expect(exact).toContainText(form.identity);
  217 |     await exact.locator("summary").click();
  218 |     await expect(exact).not.toHaveAttribute("open", "");
  219 | 
  220 |     const [birthRequest] = await Promise.all([
  221 |       page.waitForRequest(
  222 |         (request) => request.url().endsWith("/api/interaction") && request.method() === "POST",
  223 |       ),
  224 |       birthButton.click(),
  225 |     ]);
  226 |     expect(birthRequest.postDataJSON()).toMatchObject({
  227 |       kind: "invoke",
  228 |       action_id: birthAction.identity,
  229 |     });
  230 |     const birthResponse = await birthRequest.response();
  231 |     expect(birthRequest.failure(), server.errors.join("")).toBeNull();
  232 |     expect(birthResponse, server.errors.join("")).not.toBeNull();
  233 |     expect(birthResponse.ok()).toBe(true);
  234 |     await expect(page.getByRole("heading", { name: "Body topology" })).toBeVisible();
  235 |     await expect(page.getByRole("button", { name: "Body", exact: true })).toBeVisible();
  236 |     await page.getByRole("button", { name: "Form", exact: true }).click();
  237 |     await expect(page.getByRole("heading", { name: "Program structure" })).toBeVisible();
  238 |     const born = await (await fetch(`${url}/api/snapshot`)).json();
  239 |     expect(born.interaction.last_request_id).toMatch(/^navigation\//);
  240 |     expect(born.interaction.last_disposition).toBe("Succeeded");
  241 |     expect(born.presentation.basis.body_id).toBeTruthy();
  242 |     expect(born.presentation.basis.checked_form_id).toBeTruthy();
  243 |     expect(born.presentation.basis.wake_id).toBeNull();
  244 |     expect(born.presentation.basis.plan_id).toBeNull();
  245 |     expect(born.presentation.basis.active_play_id).toBeNull();
  246 |     expect(born.parts.parts).toHaveLength(1);
  247 |     expect(born.parts.parts[0].state).toBe("Here");
  248 |     expect(born.presentation.subjects.some(({ role }) => role === "Form")).toBe(true);
  249 | 
  250 |     const stale = await page.evaluate(async ({ presentationId, revision, subject }) =>
  251 |       (await fetch("/api/front-door-transition", {
  252 |         method: "POST",
  253 |         headers: { "Content-Type": "application/json" },
  254 |         body: JSON.stringify({
  255 |           presentation_id: presentationId,
  256 |           revision,
  257 |           action: "birth",
  258 |           subject,
  259 |         }),
  260 |       })).json(),
  261 |     { presentationId: opened.presentation.identity, revision: opened.revision, subject: form.identity });
  262 |     expect(stale.interaction.last_disposition).toBe("Refused(StalePresentation)");
  263 |     expect(stale.presentation.basis.body_id).toBe(born.presentation.basis.body_id);
  264 | 
  265 |     const wakeAction = born.presentation.actions.find(
  266 |       ({ intent }) => intent === "conduit.intent/wake@1",
  267 |     );
  268 |     expect(wakeAction.availability).toBe("Available");
  269 |     if(await page.locator("body").getAttribute("data-inspector-open")!=="true")await page.locator("#toggle-inspector").click();
  270 |     await page.locator("#structured-navigator").evaluate(element=>{element.closest("details").open=true;});
  271 |     await page.locator(`#structured-navigator input[type="radio"][data-subject="${wakeAction.target}"]`).click();
  272 |     await Promise.all([
  273 |       page.waitForResponse(
  274 |         (response) => response.url().endsWith("/api/interaction") && response.request().method() === "POST",
  275 |       ),
  276 |       page.getByRole("button", { name: "WAKE", exact: true }).click(),
  277 |     ]);
  278 |     const awakened = await (await fetch(`${url}/api/snapshot`)).json();
  279 |     expect(awakened.interaction.last_disposition).toBe("Succeeded");
  280 |     expect(awakened.presentation.basis.body_id).toBe(born.presentation.basis.body_id);
  281 |     expect(awakened.presentation.basis.wake_id).toBeTruthy();
  282 |     expect(awakened.presentation.basis.plan_id).toBeNull();
  283 | 
  284 |     await page.getByRole("button", { name: "Plan current form" }).click();
  285 |     await expect(page.locator("#front-door-feedback")).toContainText("Plan Succeeded");
  286 |     const planned = await (await fetch(`${url}/api/snapshot`)).json();
```