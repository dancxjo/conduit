import { spawn } from "node:child_process";
import { expect } from "@playwright/test";

function awaitUrl(child, pattern, label) {
  let output = "";
  return new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error(`${label} was not ready\n${output}`)), 10_000);
    const inspect = (chunk) => {
      output += chunk.toString();
      const match = output.match(pattern);
      if (match) {
        clearTimeout(timeout);
        resolve(match[1]);
      }
    };
    child.stdout.on("data", inspect);
    child.stderr.on("data", inspect);
    child.once("exit", (code) => {
      clearTimeout(timeout);
      reject(new Error(`${label} exited (${code})\n${output}`));
    });
  });
}

export async function startTour() {
  const child = spawn("target/debug/conduit-browser-host", ["--application", "target/tour-product", "--mount", "/tour/", "--no-open"], {
    cwd: new URL("../..", import.meta.url).pathname,
    env: process.env,
    stdio: ["ignore", "pipe", "pipe"],
  });
  const url = await awaitUrl(child, /CONDUIT_BROWSER_HOST_URL=(http:\/\/127\.0\.0\.1:\d+\/tour\/)/, "executable Tour");
  return { child, url };
}

export async function startStaticProduct(root, mount = "/") {
  const child = spawn("node", ["proof/browser/static-server.mjs", "0", root, mount], {
    cwd: new URL("../..", import.meta.url).pathname,
    stdio: ["ignore", "pipe", "pipe"],
  });
  const url = await awaitUrl(child, /CONDUIT_STATIC_SERVER_URL=(http:\/\/127\.0\.0\.1:\d+\/\S*)/, "staged product");
  return { child, url };
}

export async function openTourStep(page, entrance, index) {
  await page.goto(entrance.url);
  await expect(page.locator("#host-state")).toHaveText("Browser Host ready");
  for (let current = 0; current < index; current += 1) {
    await page.getByRole("button", { name: "Next" }).click();
    await expect(page.locator('[data-application-key="progress"]')).toHaveText(new RegExp(`^Page ${current + 2} of \\d+$`));
  }
  await expect(page.locator('[data-application-key="progress"]')).toHaveText(new RegExp(`^Page ${index + 1} of \\d+$`));
}
