# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: presentation-nucleus.spec.mjs >> portable presentation nucleus executes in WASM and manifests in Chromium
- Location: proof/browser/presentation-nucleus.spec.mjs:3:1

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
    - locator resolved to <p id="result">running</p>
    - unexpected value "running"
    13 × locator resolved to <p id="result">failed: application presentation refused: unknown…</p>
       - unexpected value "failed: application presentation refused: unknown-component"

```

```yaml
- paragraph: "failed: application presentation refused: unknown-component"
```

# Test source

```ts
  1   | import { expect, test } from "@playwright/test";
  2   | 
  3   | test("portable presentation nucleus executes in WASM and manifests in Chromium", async ({ page }) => {
  4   |   const failures = [];
  5   |   page.on("pageerror", (error) => failures.push(error.stack ?? String(error)));
  6   |   page.on("console", (message) => {
  7   |     if (message.type() === "error") failures.push(message.text());
  8   |   });
  9   | 
  10  |   await page.goto("/proof/browser/presentation-nucleus.test.html");
> 11  |   await expect(page.locator("#result")).toHaveText("ok");
      |                                         ^ Error: expect(locator).toHaveText(expected) failed
  12  |   await expect(page.locator("#nucleus [data-layout-index]")).toHaveCount(3);
  13  |   await expect(page.locator("#nucleus [data-graphics-kind]")).toHaveCount(3);
  14  |   await expect(page.locator("#nucleus [data-graphics-kind=text]")).toHaveText("ready");
  15  |   await expect(page.locator("#nucleus [data-graphics-kind=text]")).toHaveAttribute("data-graphics-text-role", "body");
  16  |   await expect(page.locator("#nucleus [data-graphics-kind=icon]")).toHaveAttribute("role", "img");
  17  |   await expect(page.locator("#nucleus [data-presentation-kind=text]")).toHaveText("STRASSE");
  18  |   await expect(page.locator("#nucleus [data-application-component=shell]")).toHaveCount(1);
  19  |   await expect(page.locator("#nucleus")).toHaveAttribute("data-application-theme", "conduit.presentation/phosphor@1");
  20  |   await expect(page.locator("#nucleus")).toHaveCSS("font-size", "16px");
  21  |   await expect(page.locator("#nucleus")).toHaveCSS("line-height", "24px");
  22  |   const themedButton = page.locator("#nucleus [data-application-component=button]");
  23  |   await expect(themedButton).toHaveCSS("border-radius", "6px");
  24  |   await themedButton.focus();
  25  |   await expect(themedButton).toHaveCSS("outline-width", "3px");
  26  |   await expect(page.locator("#nucleus [data-application-key=heading] > h3")).toHaveText("Browser Host presentation");
  27  |   await expect(page.locator("#nucleus [data-application-key=heading]")).toHaveAttribute("data-application-evidence", "succeeded");
  28  |   const structured = page.locator("#nucleus [data-presentation-kind=structured-info]");
  29  |   await expect(structured).toHaveAttribute("data-schema", "education/feedback@1");
  30  |   await expect(structured).toHaveAttribute("data-variant", "passed");
  31  |   await expect(structured).toHaveAttribute("data-quantity-unit", "ratio/percent");
  32  |   await expect(structured).toHaveAttribute("data-quantity", "88e0");
  33  |   expect(failures).toEqual([]);
  34  |   expect(await page.evaluate(() => globalThis.__conduitPresentationNucleus)).toEqual({
  35  |     layoutChildren: 3,
  36  |     graphicsKinds: [1, 2, 3],
  37  |     text: "STRASSE",
  38  |     structured: {
  39  |       schema: "education/feedback@1",
  40  |       variant: "passed",
  41  |       quantityUnit: "ratio/percent",
  42  |       quantity: "88e0",
  43  |     },
  44  |     application: {
  45  |       revision: 1,
  46  |       theme: "conduit.presentation/phosphor@1",
  47  |       heading: "Browser Host presentation",
  48  |       evidenceIdentity: "sign-nucleus-1",
  49  |       evidenceProvenance: "plan-nucleus-1/play-nucleus-1",
  50  |       artifactTag: "ARTICLE",
  51  |       rawLanguage: "json",
  52  |       rawText: '{"content":"<script>inert</script>"}',
  53  |       executableScripts: 0,
  54  |       action: "nucleus.continue",
  55  |       encodedEventBytes: 27,
  56  |       queuedEvents: 0,
  57  |       pressureRefusal: "queue-pressure",
  58  |       malformedRefusal: "unsupported-version",
  59  |       unchangedAfterRefusal: true,
  60  |       controlValue: "gear edited",
  61  |       controlLabel: "plot source",
  62  |       controlBytePressureRefusal: "queue-pressure",
  63  |       semantic: {
  64  |         panelTag: "SECTION",
  65  |         busy: "busy",
  66  |         busyRefusal: "action-busy",
  67  |         unavailable: "unavailable",
  68  |         unavailableRefusal: "unavailable-action",
  69  |         warning: "warning",
  70  |         failureRole: "alert",
  71  |         disclosureOpen: true,
  72  |         disclosureEvents: 0,
  73  |         staleRefusal: "stale-revision",
  74  |         retiredVersion: 9,
  75  |         retiredRefusal: "unsupported-version",
  76  |       },
  77  |     },
  78  |     missingContext: true,
  79  |   });
  80  | });
  81  | 
  82  | test("shared plots and navigation preserve exact keyboard interaction across revisions", async ({ page }) => {
  83  |   await page.goto("/proof/browser/presentation-nucleus.test.html");
  84  |   await expect(page.locator("#result")).toHaveText("ok");
  85  |   const field = page.locator('[data-application-key="source-field"]');
  86  |   const label = field.locator('[data-application-key="source-label"]');
  87  |   const control = field.locator('[data-application-key="source-control"]');
  88  |   const help = field.locator('[data-application-key="source-help"]');
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
```