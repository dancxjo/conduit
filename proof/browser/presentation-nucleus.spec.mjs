import { expect, test } from "@playwright/test";

test("portable presentation nucleus executes in WASM and manifests in Chromium", async ({ page }) => {
  const failures = [];
  page.on("pageerror", (error) => failures.push(error.stack ?? String(error)));
  page.on("console", (message) => {
    if (message.type() === "error") failures.push(message.text());
  });

  await page.goto("/proof/browser/presentation-nucleus.test.html");
  await expect(page.locator("#result")).toHaveText("ok");
  await expect(page.locator("#nucleus [data-layout-index]")).toHaveCount(3);
  await expect(page.locator("#nucleus [data-graphics-kind]")).toHaveCount(3);
  await expect(page.locator("#nucleus [data-graphics-kind=text]")).toHaveText("ready");
  await expect(page.locator("#nucleus [data-graphics-kind=icon]")).toHaveAttribute("role", "img");
  await expect(page.locator("#nucleus [data-presentation-kind=text]")).toHaveText("STRASSE");
  await expect(page.locator("#nucleus [data-application-component=shell]")).toHaveCount(1);
  await expect(page.locator("#nucleus")).toHaveAttribute("data-application-theme", "conduit.presentation/phosphor@1");
  await expect(page.locator("#nucleus")).toHaveCSS("font-size", "16px");
  await expect(page.locator("#nucleus")).toHaveCSS("line-height", "24px");
  const themedButton = page.locator("#nucleus [data-application-component=button]");
  await expect(themedButton).toHaveCSS("border-radius", "6px");
  await themedButton.focus();
  await expect(themedButton).toHaveCSS("outline-width", "3px");
  await expect(page.locator("#nucleus [data-application-key=heading] > h3")).toHaveText("Browser Host presentation");
  await expect(page.locator("#nucleus [data-application-key=heading]")).toHaveAttribute("data-application-evidence", "succeeded");
  const structured = page.locator("#nucleus [data-presentation-kind=structured-info]");
  await expect(structured).toHaveAttribute("data-schema", "education/feedback@1");
  await expect(structured).toHaveAttribute("data-variant", "passed");
  await expect(structured).toHaveAttribute("data-quantity-unit", "ratio/percent");
  await expect(structured).toHaveAttribute("data-quantity", "88");
  expect(failures).toEqual([]);
  expect(await page.evaluate(() => globalThis.__conduitPresentationNucleus)).toEqual({
    layoutChildren: 3,
    graphicsKinds: [1, 2, 3],
    text: "STRASSE",
    structured: {
      schema: "education/feedback@1",
      variant: "passed",
      quantityUnit: "ratio/percent",
      quantity: 88,
    },
    application: {
      revision: 1,
      theme: "conduit.presentation/phosphor@1",
      heading: "Browser Host presentation",
      evidenceIdentity: "sign-nucleus-1",
      evidenceProvenance: "plan-nucleus-1/play-nucleus-1",
      artifactTag: "ARTICLE",
      rawLanguage: "json",
      rawText: '{"content":"<script>inert</script>"}',
      executableScripts: 0,
      action: "nucleus.continue",
      encodedEventBytes: 27,
      queuedEvents: 0,
      pressureRefusal: "queue-pressure",
      malformedRefusal: "unsupported-version",
      unchangedAfterRefusal: true,
      controlValue: "gear edited",
      controlLabel: "Form source",
      controlBytePressureRefusal: "queue-pressure",
      semantic: {
        panelTag: "SECTION",
        busy: "busy",
        busyRefusal: "action-busy",
        unavailable: "unavailable",
        unavailableRefusal: "unavailable-action",
        warning: "warning",
        failureRole: "alert",
        disclosureOpen: true,
        disclosureEvents: 0,
        staleRefusal: "stale-revision",
        retiredVersion: 7,
        retiredRefusal: "unsupported-version",
      },
    },
    missingContext: true,
  });
});

test("shared forms and navigation preserve exact keyboard interaction across revisions", async ({ page }) => {
  await page.goto("/proof/browser/presentation-nucleus.test.html");
  await expect(page.locator("#result")).toHaveText("ok");
  const field = page.locator('[data-application-key="source-field"]');
  const label = field.locator('[data-application-key="source-label"]');
  const control = field.locator('[data-application-key="source-control"]');
  const help = field.locator('[data-application-key="source-help"]');
  const error = field.locator('[data-application-key="source-error"]');
  await expect(label).toHaveAttribute("for", await control.getAttribute("id"));
  await expect(field.getByLabel("Form source")).toHaveCount(1);
  await expect(control).toHaveAttribute("aria-describedby", `${await help.getAttribute("id")} ${await error.getAttribute("id")}`);
  await expect(control).toHaveAttribute("aria-errormessage", await error.getAttribute("id"));
  await expect(control).toHaveAttribute("aria-invalid", "true");

  await control.focus();
  await control.evaluate((element) => element.setSelectionRange(2, 7, "forward"));
  await page.evaluate(() => globalThis.__conduitFocusProof.rerender());
  const revised = field.locator('[data-application-key="source-control"]');
  await expect(revised).toBeFocused();
  expect(await revised.evaluate((element) => ({ start: element.selectionStart, end: element.selectionEnd, direction: element.selectionDirection })))
    .toEqual({ start: 2, end: 7, direction: "forward" });
  await expect(revised).not.toHaveAttribute("aria-invalid", "true");
  await page.evaluate(() => globalThis.__conduitFocusProof.dispatchDetached());
  expect(await page.evaluate(() => globalThis.__conduitFocusProof.host.lastRefusal("focus"))).toBe("stale-revision");
  expect(await page.evaluate(() => globalThis.__conduitFocusProof.host.nextEvent("focus"))).toBeNull();

  const navigation = page.locator('[data-application-key="navigation"]');
  const previous = navigation.locator('[data-application-key="page-one"]');
  const next = navigation.locator('[data-application-key="page-two"]');
  await expect(next).toHaveAttribute("tabindex", "0");
  await expect(previous).toHaveAttribute("tabindex", "-1");
  await next.focus();
  await next.press("Home");
  await expect(previous).toBeFocused();
  await previous.press("End");
  await expect(next).toBeFocused();
  await next.press("ArrowLeft");
  await expect(previous).toBeFocused();
  await previous.press("ArrowDown");
  await expect(next).toBeFocused();

  const stepper = page.locator('[data-application-key="stepper"]');
  await expect(stepper).toHaveAttribute("data-application-current", "2");
  await expect(stepper.locator('[data-application-key="step-two"]')).toHaveAttribute("tabindex", "0");
  const progress = page.locator('progress[data-application-key="progress"]');
  await expect(progress).toHaveAttribute("value", "2");
  await expect(progress).toHaveAttribute("max", "3");
  await revised.focus();
  await page.evaluate(() => globalThis.__conduitFocusProof.removeFocused());
  await expect(page.locator('[data-application-key="page-two"]')).toBeFocused();
});

test("semantic product links retain their visible label and current destination", async ({ page }) => {
  await page.goto("/proof/browser/presentation-nucleus.test.html");
  await page.evaluate(async () => {
    const { encodeApplicationView, manifestApplicationView } = await import(
      "/targets/browser/host/assets/application-presentation.mjs"
    );
    const root = document.createElement("div");
    root.id = "navigation-link-proof";
    document.body.append(root);
    manifestApplicationView(encodeApplicationView({
      revision: 1,
      actions: [],
      nodes: [
        { parent: null, component: "shell", key: "shell", text: "", action: null },
        { parent: 0, component: "navigation", key: "product-navigation", text: "Conduit products", value: "tour", valueCapacity: 16, action: null },
        { parent: 1, component: "navigation-link", key: "tour", text: "Tour", value: "tour", valueCapacity: 16, action: null },
      ],
    }), root);
  });
  const link = page.locator('#navigation-link-proof [data-application-key="tour"]');
  await expect(link).toHaveText("Tour");
  await expect(link).toHaveAttribute("href", "/conduit/tour/");
  await expect(link).toHaveAttribute("aria-current", "page");
});

test("one finite theme mechanism preserves contrast and responsive layout across products", async ({ page }) => {
  await page.setViewportSize({ width: 680, height: 900 });
  await page.goto("/proof/browser/presentation-nucleus.test.html");
  await expect(page.locator("#result")).toHaveText("ok");
  for (const product of ["tour", "creche", "patchbay"]) {
    const root = page.locator(`[data-conduit-product="${product}"]`);
    await expect(root).toHaveAttribute("data-application-theme", "conduit.presentation/phosphor@1");
    await expect(root.locator('[data-application-component="panel"]')).toHaveCSS("border-radius", "9px");
    await expect(root.locator('[data-application-component="panel"]')).toHaveCSS("border-color", "rgb(244, 196, 0)");
    await expect(root.locator('[data-application-component="grid"]')).toHaveCSS("grid-template-columns", /\d+px/);
    expect(await root.evaluate((element) => getComputedStyle(element).getPropertyValue("--conduit-structure-secondary").trim()))
      .toBe("#f4c400");
  }
});
