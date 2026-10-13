# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: presentation-nucleus.spec.mjs >> one finite theme mechanism preserves contrast and responsive layout across products
- Location: proof/browser/presentation-nucleus.spec.mjs:186:1

# Error details

```
Error: expect(locator).toHaveText(expected) failed

Locator:  locator('#result')
Expected: "ok"
Received: "failed: application presentation refused: unknown-component"
Timeout:  5000ms

Call log:
  - Expect "toHaveText" with timeout 5000ms
  - waiting for locator('#result')
    13 × locator resolved to <p id="result">failed: application presentation refused: unknown…</p>
       - unexpected value "failed: application presentation refused: unknown-component"

```

```yaml
- paragraph: "failed: application presentation refused: unknown-component"
```

# Test source

```ts
  89  |   const error = field.locator('[data-application-key="source-error"]');
  90  |   await expect(label).toHaveAttribute("for", await control.getAttribute("id"));
  91  |   await expect(field.getByLabel("plot source")).toHaveCount(1);
  92  |   await expect(control).toHaveAttribute("aria-describedby", `${await help.getAttribute("id")} ${await error.getAttribute("id")}`);
  93  |   await expect(control).toHaveAttribute("aria-errormessage", await error.getAttribute("id"));
  94  |   await expect(control).toHaveAttribute("aria-invalid", "true");
  95  | 
  96  |   await control.focus();
  97  |   await control.evaluate((element) => element.setSelectionRange(2, 7, "forward"));
  98  |   await page.evaluate(() => globalThis.__conduitFocusProof.rerender());
  99  |   const revised = field.locator('[data-application-key="source-control"]');
  100 |   await expect(revised).toBeFocused();
  101 |   expect(await revised.evaluate((element) => ({ start: element.selectionStart, end: element.selectionEnd, direction: element.selectionDirection })))
  102 |     .toEqual({ start: 2, end: 7, direction: "forward" });
  103 |   await expect(revised).not.toHaveAttribute("aria-invalid", "true");
  104 |   await page.evaluate(() => globalThis.__conduitFocusProof.dispatchDetached());
  105 |   expect(await page.evaluate(() => globalThis.__conduitFocusProof.host.lastRefusal("focus"))).toBe("stale-revision");
  106 |   expect(await page.evaluate(() => globalThis.__conduitFocusProof.host.nextEvent("focus"))).toBeNull();
  107 | 
  108 |   const navigation = page.locator('[data-application-key="navigation"]');
  109 |   const previous = navigation.locator('[data-application-key="page-one"]');
  110 |   const next = navigation.locator('[data-application-key="page-two"]');
  111 |   await expect(next).toHaveAttribute("tabindex", "0");
  112 |   await expect(previous).toHaveAttribute("tabindex", "-1");
  113 |   await next.focus();
  114 |   await next.press("Home");
  115 |   await expect(previous).toBeFocused();
  116 |   await previous.press("End");
  117 |   await expect(next).toBeFocused();
  118 |   await next.press("ArrowLeft");
  119 |   await expect(previous).toBeFocused();
  120 |   await previous.press("ArrowDown");
  121 |   await expect(next).toBeFocused();
  122 | 
  123 |   const stepper = page.locator('[data-application-key="stepper"]');
  124 |   await expect(stepper).toHaveAttribute("data-application-current", "2");
  125 |   await expect(stepper.locator('[data-application-key="step-two"]')).toHaveAttribute("tabindex", "0");
  126 |   const progress = page.locator('progress[data-application-key="progress"]');
  127 |   await expect(progress).toHaveAttribute("value", "2");
  128 |   await expect(progress).toHaveAttribute("max", "3");
  129 |   await revised.focus();
  130 |   await page.evaluate(() => globalThis.__conduitFocusProof.removeFocused());
  131 |   await expect(page.locator('[data-application-key="page-two"]')).toBeFocused();
  132 | });
  133 | 
  134 | test("semantic product and same-site links retain exact bounded destinations without actions", async ({ page }) => {
  135 |   await page.goto("/proof/browser/presentation-nucleus.test.html");
  136 |   await page.evaluate(async () => {
  137 |     const { encodeApplicationView, manifestApplicationView } = await import(
  138 |       "/targets/browser/host/assets/application-presentation.mjs"
  139 |     );
  140 |     const root = document.createElement("div");
  141 |     root.id = "navigation-link-proof";
  142 |     document.body.append(root);
  143 |     manifestApplicationView(encodeApplicationView({
  144 |       revision: 1,
  145 |       actions: [],
  146 |       nodes: [
  147 |         { parent: null, component: "shell", key: "shell", text: "", action: null },
  148 |         { parent: 0, component: "navigation", key: "product-navigation", text: "Conduit products", value: "tour", valueCapacity: 16, action: null },
  149 |         { parent: 1, component: "navigation-link", key: "tour", text: "Tour", value: "tour", valueCapacity: 16, action: null },
  150 |       ],
  151 |     }), root);
  152 |     const handoff = document.createElement("div");
  153 |     handoff.id = "same-site-link-proof";
  154 |     document.body.append(handoff);
  155 |     manifestApplicationView(encodeApplicationView({
  156 |       revision: 1,
  157 |       actions: [],
  158 |       nodes: [{
  159 |         parent: null, component: "link", key: "handoff", text: "Add to new body",
  160 |         value: "/conduit/workspace/?plot=memory_lantern", valueCapacity: 2_048, action: null,
  161 |       }],
  162 |     }), handoff);
  163 |     let externalRefusal = "";
  164 |     try {
  165 |       encodeApplicationView({
  166 |         revision: 1,
  167 |         actions: [],
  168 |         nodes: [{
  169 |           parent: null, component: "link", key: "external", text: "External",
  170 |           value: "https://example.invalid/", valueCapacity: 2_048, action: null,
  171 |         }],
  172 |       });
  173 |     } catch (error) { externalRefusal = error.code; }
  174 |     globalThis.__sameSiteLinkRefusal = externalRefusal;
  175 |   });
  176 |   const link = page.locator('#navigation-link-proof [data-application-key="tour"]');
  177 |   await expect(link).toHaveText("Tour");
  178 |   await expect(link).toHaveAttribute("href", "/conduit/workspace/");
  179 |   await expect(link).toHaveAttribute("aria-current", "page");
  180 |   const handoff = page.locator('#same-site-link-proof [data-application-key="handoff"]');
  181 |   await expect(handoff).toHaveAttribute("href", /\/conduit\/workspace\/\?plot=memory_lantern$/u);
  182 |   expect(await handoff.evaluate((element) => element.onclick)).toBeNull();
  183 |   expect(await page.evaluate(() => globalThis.__sameSiteLinkRefusal)).toBe("invalid-control-value");
  184 | });
  185 | 
  186 | test("one finite theme mechanism preserves contrast and responsive layout across products", async ({ page }) => {
  187 |   await page.setViewportSize({ width: 680, height: 900 });
  188 |   await page.goto("/proof/browser/presentation-nucleus.test.html");
> 189 |   await expect(page.locator("#result")).toHaveText("ok");
      |                                         ^ Error: expect(locator).toHaveText(expected) failed
  190 |   for (const product of ["tour", "creche", "patchbay"]) {
  191 |     const root = page.locator(`[data-conduit-product="${product}"]`);
  192 |     await expect(root).toHaveAttribute("data-application-theme", "conduit.presentation/phosphor@1");
  193 |     await expect(root.locator('[data-application-component="panel"]')).toHaveCSS("border-radius", "9px");
  194 |     await expect(root.locator('[data-application-component="panel"]')).toHaveCSS("border-color", "rgb(244, 196, 0)");
  195 |     await expect(root.locator('[data-application-component="grid"]')).toHaveCSS("grid-template-columns", /\d+px/);
  196 |     expect(await root.evaluate((element) => getComputedStyle(element).getPropertyValue("--conduit-structure-secondary").trim()))
  197 |       .toBe("#f4c400");
  198 |   }
  199 | });
  200 | 
```