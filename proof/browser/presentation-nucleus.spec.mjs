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
  await expect(page.locator("#nucleus [data-application-key=heading]")).toHaveText("Browser Host presentation");
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
      action: "nucleus.continue",
      encodedEventBytes: 25,
      queuedEvents: 0,
      pressureRefusal: "queue-pressure",
      malformedRefusal: "unsupported-version",
      unchangedAfterRefusal: true,
    },
    missingContext: true,
  });
});
