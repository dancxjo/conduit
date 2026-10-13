# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: presentation-nucleus.spec.mjs >> one finite theme mechanism preserves contrast and responsive layout across products
- Location: proof/browser/presentation-nucleus.spec.mjs:188:1

# Error details

```
Error: expect(locator).toHaveText(expected) failed

Locator:  locator('#result')
Expected: "ok"
Received: "failed: invalid graphics scene"
Timeout:  5000ms

Call log:
  - Expect "toHaveText" with timeout 5000ms
  - waiting for locator('#result')
    13 × locator resolved to <p id="result">failed: invalid graphics scene</p>
       - unexpected value "failed: invalid graphics scene"

```

```yaml
- paragraph: "failed: invalid graphics scene"
```

# Test source

```ts
  91  |   const error = field.locator('[data-application-key="source-error"]');
  92  |   await expect(label).toHaveAttribute("for", await control.getAttribute("id"));
  93  |   await expect(field.getByLabel("plot source")).toHaveCount(1);
  94  |   await expect(control).toHaveAttribute("aria-describedby", `${await help.getAttribute("id")} ${await error.getAttribute("id")}`);
  95  |   await expect(control).toHaveAttribute("aria-errormessage", await error.getAttribute("id"));
  96  |   await expect(control).toHaveAttribute("aria-invalid", "true");
  97  | 
  98  |   await control.focus();
  99  |   await control.evaluate((element) => element.setSelectionRange(2, 7, "forward"));
  100 |   await page.evaluate(() => globalThis.__conduitFocusProof.rerender());
  101 |   const revised = field.locator('[data-application-key="source-control"]');
  102 |   await expect(revised).toBeFocused();
  103 |   expect(await revised.evaluate((element) => ({ start: element.selectionStart, end: element.selectionEnd, direction: element.selectionDirection })))
  104 |     .toEqual({ start: 2, end: 7, direction: "forward" });
  105 |   await expect(revised).not.toHaveAttribute("aria-invalid", "true");
  106 |   await page.evaluate(() => globalThis.__conduitFocusProof.dispatchDetached());
  107 |   expect(await page.evaluate(() => globalThis.__conduitFocusProof.host.lastRefusal("focus"))).toBe("stale-revision");
  108 |   expect(await page.evaluate(() => globalThis.__conduitFocusProof.host.nextEvent("focus"))).toBeNull();
  109 | 
  110 |   const navigation = page.locator('[data-application-key="navigation"]');
  111 |   const previous = navigation.locator('[data-application-key="page-one"]');
  112 |   const next = navigation.locator('[data-application-key="page-two"]');
  113 |   await expect(next).toHaveAttribute("tabindex", "0");
  114 |   await expect(previous).toHaveAttribute("tabindex", "-1");
  115 |   await next.focus();
  116 |   await next.press("Home");
  117 |   await expect(previous).toBeFocused();
  118 |   await previous.press("End");
  119 |   await expect(next).toBeFocused();
  120 |   await next.press("ArrowLeft");
  121 |   await expect(previous).toBeFocused();
  122 |   await previous.press("ArrowDown");
  123 |   await expect(next).toBeFocused();
  124 | 
  125 |   const stepper = page.locator('[data-application-key="stepper"]');
  126 |   await expect(stepper).toHaveAttribute("data-application-current", "2");
  127 |   await expect(stepper.locator('[data-application-key="step-two"]')).toHaveAttribute("tabindex", "0");
  128 |   const progress = page.locator('progress[data-application-key="progress"]');
  129 |   await expect(progress).toHaveAttribute("value", "2");
  130 |   await expect(progress).toHaveAttribute("max", "3");
  131 |   await revised.focus();
  132 |   await page.evaluate(() => globalThis.__conduitFocusProof.removeFocused());
  133 |   await expect(page.locator('[data-application-key="page-two"]')).toBeFocused();
  134 | });
  135 | 
  136 | test("semantic product and same-site links retain exact bounded destinations without actions", async ({ page }) => {
  137 |   await page.goto("/proof/browser/presentation-nucleus.test.html");
  138 |   await page.evaluate(async () => {
  139 |     const { encodeApplicationView, manifestApplicationView } = await import(
  140 |       "/targets/browser/host/assets/application-presentation.mjs"
  141 |     );
  142 |     const root = document.createElement("div");
  143 |     root.id = "navigation-link-proof";
  144 |     document.body.append(root);
  145 |     manifestApplicationView(encodeApplicationView({
  146 |       revision: 1,
  147 |       actions: [],
  148 |       nodes: [
  149 |         { parent: null, component: "shell", key: "shell", text: "", action: null },
  150 |         { parent: 0, component: "navigation", key: "product-navigation", text: "Conduit products", value: "tour", valueCapacity: 16, action: null },
  151 |         { parent: 1, component: "navigation-link", key: "tour", text: "Tour", value: "tour", valueCapacity: 16, action: null },
  152 |       ],
  153 |     }), root);
  154 |     const handoff = document.createElement("div");
  155 |     handoff.id = "same-site-link-proof";
  156 |     document.body.append(handoff);
  157 |     manifestApplicationView(encodeApplicationView({
  158 |       revision: 1,
  159 |       actions: [],
  160 |       nodes: [{
  161 |         parent: null, component: "link", key: "handoff", text: "Add to new body",
  162 |         value: "/conduit/workspace/?plot=memory_lantern", valueCapacity: 2_048, action: null,
  163 |       }],
  164 |     }), handoff);
  165 |     let externalRefusal = "";
  166 |     try {
  167 |       encodeApplicationView({
  168 |         revision: 1,
  169 |         actions: [],
  170 |         nodes: [{
  171 |           parent: null, component: "link", key: "external", text: "External",
  172 |           value: "https://example.invalid/", valueCapacity: 2_048, action: null,
  173 |         }],
  174 |       });
  175 |     } catch (error) { externalRefusal = error.code; }
  176 |     globalThis.__sameSiteLinkRefusal = externalRefusal;
  177 |   });
  178 |   const link = page.locator('#navigation-link-proof [data-application-key="tour"]');
  179 |   await expect(link).toHaveText("Tour");
  180 |   await expect(link).toHaveAttribute("href", "/conduit/workspace/");
  181 |   await expect(link).toHaveAttribute("aria-current", "page");
  182 |   const handoff = page.locator('#same-site-link-proof [data-application-key="handoff"]');
  183 |   await expect(handoff).toHaveAttribute("href", /\/conduit\/workspace\/\?plot=memory_lantern$/u);
  184 |   expect(await handoff.evaluate((element) => element.onclick)).toBeNull();
  185 |   expect(await page.evaluate(() => globalThis.__sameSiteLinkRefusal)).toBe("invalid-control-value");
  186 | });
  187 | 
  188 | test("one finite theme mechanism preserves contrast and responsive layout across products", async ({ page }) => {
  189 |   await page.setViewportSize({ width: 680, height: 900 });
  190 |   await page.goto("/proof/browser/presentation-nucleus.test.html");
> 191 |   await expect(page.locator("#result")).toHaveText("ok");
      |                                         ^ Error: expect(locator).toHaveText(expected) failed
  192 |   for (const product of ["tour", "creche", "patchbay"]) {
  193 |     const root = page.locator(`[data-conduit-product="${product}"]`);
  194 |     await expect(root).toHaveAttribute("data-application-theme", "conduit.presentation/phosphor@1");
  195 |     await expect(root.locator('[data-application-component="panel"]')).toHaveCSS("border-radius", "9px");
  196 |     await expect(root.locator('[data-application-component="panel"]')).toHaveCSS("border-color", "rgb(244, 196, 0)");
  197 |     await expect(root.locator('[data-application-component="grid"]')).toHaveCSS("grid-template-columns", /\d+px/);
  198 |     expect(await root.evaluate((element) => getComputedStyle(element).getPropertyValue("--conduit-structure-secondary").trim()))
  199 |       .toBe("#f4c400");
  200 |   }
  201 | });
  202 | 
```