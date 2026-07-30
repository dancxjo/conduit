import { expect, test } from "@playwright/test";

test("runs a production lesson in the resolved browser worker", async ({ page }) => {
  const failures = [];
  page.on("pageerror", (error) => failures.push(error.stack ?? String(error)));
  page.on("console", (message) => {
    if (message.type() === "error") failures.push(message.text());
  });

  await page.goto("/tour/public/index.html?autorun");
  await expect(page.locator("#result")).toContainText("Hello from the Tour.", {
    timeout: 20_000,
  });
  await expect(page.locator("#result")).toContainText(
    "Evidence: 2 nodes, 1 cords conducted.",
  );
  await expect(page.locator("#execution-note")).toContainText(
    "exact dedicated-worker placement",
  );
  await expect(page.locator("#plan")).toContainText(
    "conduit/hosted-literal-v1",
  );
  await expect(page.locator("#plan")).toContainText("bound-in-this-plan");
  await expect(page.locator("#evidence")).toContainText('"event_kind": "terminal"');
  await expect(page.locator("#evidence")).toContainText('"terminal_cause": "succeeded"');
  expect(failures).toEqual([]);
});

test("preserves a recoverable draft across reset", async ({ page }) => {
  await page.goto("/tour/public/index.html");
  const source = page.locator("#source");
  await expect(source).toHaveValue(/Hello from the Tour\./);
  await source.fill((await source.inputValue()).replace("Hello from the Tour.", "Recover me."));
  await page.locator("#reset").click();
  await expect(source).toHaveValue(/Hello from the Tour\./);
  await page.locator("#undo-reset").click();
  await expect(source).toHaveValue(/Recover me\./);
});

test("covers Chapters 0-3 and exposes production topology projections", async ({ page }) => {
  await page.goto("/tour/public/index.html");
  await expect(page.locator("#lessons > li")).toHaveCount(15);
  await page.getByRole("button", { name: "Inside / outside" }).click();
  await expect(page.locator("#source")).toHaveValue(/example\/upper-box/);
  await page.locator("#expanded-view").click();
  await expect(page.locator("#topology")).toContainText(
    "box.worker : conduit.std/uppercase",
  );
  await page.locator("#logical-view").click();
  await expect(page.locator("#topology")).toContainText(
    "composite box : example/upper-box",
  );
});

test("accepts a semantically correct alternate solution", async ({ page }) => {
  await page.goto("/tour/public/index.html");
  const source = page.locator("#source");
  await expect(source).toHaveValue(/Hello from the Tour\./);
  await source.fill(
    (await source.inputValue())
      .replace("node greeting ", "node salutation ")
      .replace("greeting.out", "salutation.out"),
  );
  await page.locator("#run").click();
  await expect(page.locator("#result")).toContainText("✓ Lesson complete!", {
    timeout: 20_000,
  });
  await expect(source).toHaveValue(/node salutation/);
});

test("uses React Flow with legacy line placement disabled", async ({ page }) => {
  await page.goto("/tour/public/index.html");
  const canvas = page.locator("#patchbay-flow-root");
  await expect(canvas).toHaveAttribute("data-renderer", "react-flow");
  await expect(canvas).toHaveAttribute("data-projection", "rust-authoritative-v1");
  await expect(canvas).toHaveAttribute("data-legacy-line-placement", "false");
  await expect(canvas).toHaveAttribute("data-node-count", "2");
  await expect(canvas).toHaveAttribute("data-edge-count", "1");
  await expect(page.locator(".conduit-faceplate-card")).toHaveCount(2, {
    timeout: 20_000,
  });
  await expect(page.locator(".availability-tag")).toHaveCount(2);
});

test("retains headless editing and execution when presentation fails", async ({ page }) => {
  await page.goto("/tour/public/index.html");
  await page.evaluate(() => {
    window.__CONDUIT_DISABLE_PATCHBAY_RENDERER__ = true;
  });
  const source = page.locator("#source");
  await expect(source).toHaveValue(/Hello from the Tour\./);
  await source.evaluate((element) => {
    element.value = element.value.replace("Hello from the Tour.", "Headless proof.");
    element.dispatchEvent(new Event("input", { bubbles: true }));
  });
  await expect(page.locator("#result")).toContainText("Valid runnable panel");
  await expect(page.locator("#cy")).toContainText("React Flow renderer unavailable.");
  await page.locator("#run").click();
  await expect(page.locator("#result")).toContainText("Headless proof.", {
    timeout: 20_000,
  });
});

test("styles cords from their projected type and pressure policy", async ({ page }) => {
  await page.goto("/tour/public/index.html");
  const edge = page.locator(".patchbay-smart-cord").first();
  await expect(edge).toHaveClass(/pressure-block/);
  await expect(edge).toHaveClass(/value-type-text/);
  await expect(edge.locator(".react-flow__edge-path")).toHaveAttribute("d", /^M/);
});

test("reference panels expose canonical contract-only status and disable Run", async ({ page }) => {
  await page.goto("/tour/public/index.html");
  await page.getByRole("button", { name: "File Copier Pipeline" }).click();
  await expect(page.locator("#runnability-state")).toContainText("contract-only");
  await expect(page.locator("#run")).toBeDisabled();
  await expect(page.locator("#result")).toContainText("CND-IMP-001");
  await expect(page.locator("#source")).toHaveValue(/node reader : std\/file-read/);
});

test("pedagogical completion is not execution evidence", async ({ page }) => {
  await page.goto("/tour/public/index.html");
  await page.getByRole("button", { name: "Pull the cord" }).click();
  await expect(page.locator("#run")).toBeDisabled();
  await page.locator("#check").click();
  await expect(page.locator("#result")).toContainText(
    "Lesson check complete (not execution evidence)",
  );
  await expect(page.locator("#evidence")).toContainText(
    '"executionEvidence": false',
  );
});

test("illustrative lessons cannot run their pedagogical target", async ({ page }) => {
  await page.goto("/tour/public/index.html");
  await page.getByRole("button", { name: "More than one port" }).click();
  await expect(page.locator("#runnability-state")).toContainText(
    "illustrative/unavailable",
  );
  await expect(page.locator("#run")).toBeDisabled();
  await expect(page.locator("#result")).toContainText("CND-CMP-006");
  await expect(page.locator("#evidence")).not.toContainText('"event_kind": "terminal"');
});
