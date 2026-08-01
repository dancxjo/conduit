import { chromium } from "playwright";

const url = process.argv[2];
const executablePath = process.env.CHROME_BIN;

if (!url || !executablePath) {
  throw new Error("browser smoke requires a Tour URL and CHROME_BIN");
}

const browser = await chromium.launch({
  executablePath,
  headless: true,
  args: ["--no-sandbox", "--disable-gpu"],
});

try {
  const page = await browser.newPage();
  await page.goto(url);

  const checks = [
    ["reader readiness", page.locator('html[data-tour-ready="true"]')],
    [
      "dedicated worker placement",
      page.locator("#execution-note").filter({
        hasText: "exact dedicated-worker placement",
      }),
    ],
    [
      "Rust-authoritative projection",
      page.locator('#patchbay-flow-root[data-projection="rust-authoritative"]'),
    ],
    [
      "layered topology",
      page.locator('#cy[data-layout-algorithm="layered"]'),
    ],
    [
      "visible greeting faceplate",
      page.locator('[data-testid="rf__node-greeting"][style*="visibility: visible"]'),
    ],
    ["bounded cord", page.locator(".react-flow__edge-path")],
    [
      "outgoing semantic port",
      page.locator('[aria-label="value, outgoing port; type std/text"]'),
    ],
    [
      "receiving semantic port",
      page.locator('[aria-label="text, receiving port; type std/text"]'),
    ],
    [
      "parser-backed source metadata",
      page.locator('[data-semantic-metadata="available"]'),
    ],
    [
      "panel syntax token",
      page.locator(".panel-token-keyword").filter({ hasText: /^panel$/ }),
    ],
    [
      "node type token",
      page.locator(".panel-token-type").filter({ hasText: /^std\/literal$/ }),
    ],
  ];

  for (const [label, locator] of checks) {
    try {
      await locator.first().waitFor({ state: "attached", timeout: 30_000 });
    } catch (error) {
      throw new Error(`Tour browser smoke did not observe ${label}`, { cause: error });
    }
  }

  const help = await page.locator("#canvas-help").textContent();
  if (!help?.includes("Drag nodes to adjust presentation layout")) {
    throw new Error("Tour browser smoke did not observe the Patchbay help contract");
  }

  console.log("Tour browser smoke passed: ready worker, semantic projection, and bounded cord.");
} finally {
  await browser.close();
}
