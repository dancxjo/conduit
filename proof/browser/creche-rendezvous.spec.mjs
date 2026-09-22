import { spawn } from "node:child_process";
import { expect, test } from "@playwright/test";
import { openCrecheStep, reviewAndBirth } from "./creche-test-actions.mjs";
import { startStaticProduct } from "./tour-test-server.mjs";

let entrance;

test.beforeEach(async () => {
  entrance = await startStaticProduct(process.env.CONDUIT_CRECHE_RENDEZVOUS_PRODUCT ?? "target/creche-rendezvous-product", "/conduit/creche/");
});

test.afterEach(() => entrance?.child.kill());

test("a rendezvous code admits one already-running raw Host through the web Crèche", async ({ page }) => {
  const running = spawn("target/debug/conduit", ["host", "rendezvous", "--timeout-seconds", "30"], {
    cwd: new URL("../..", import.meta.url).pathname,
    stdio: ["ignore", "pipe", "pipe"],
  });
  try {
    const code = await rendezvousCode(running);
    await page.goto(entrance.url);
    await expect(page.locator("#host-state")).toHaveText("Crèche ready");
    await reviewAndBirth(page);
    await openCrecheStep(page, "3. Physical Host");
    const runner = page.locator(".physical-host-runner");
    await runner.locator('[data-application-key="physical-target"]').selectOption("std/x86_64/computer");
    await runner.locator('[data-application-key="physical-mode"]').selectOption("attach-running");
    const input = runner.getByLabel("Running host rendezvous code");
    await input.fill(code);
    await input.press("Tab");
    await expect(runner.locator('[data-application-key="physical-stage-obtain"]')).toContainText("attachment");

    await runner.getByRole("button", { name: "Bind Body invitation" }).click();
    await expect(runner.locator('[data-application-key="physical-stage-bind"]')).not.toContainText("waiting");
    await runner.getByRole("button", { name: "Realize selected host" }).click();
    await expect(runner.locator('[data-application-key="physical-stage-realize"]')).toContainText("InvitationDelivered");
    await runner.getByRole("button", { name: "Observe Boot and join" }).click();
    await expect(runner.locator('[data-application-key="physical-stage-observe"]')).not.toContainText("waiting");
    await runner.getByRole("button", { name: "Admit Part and offers" }).click();
    await expect(runner.locator('[data-application-key="physical-stage-admit"]')).toContainText("revision");
    await expect(runner.locator('[data-application-slot="physical-status"]')).toContainText("Physical Part admitted");

    const evidence = JSON.parse((await runner.locator(".physical-evidence details code").allTextContents()).join(""));
    expect(evidence).toMatchObject({
      target: { id: "std/x86_64/computer" },
      intention: { mode: "attach-running", supported: true },
      obtainment: { line_id: "conduit-line/loopback-websocket@1", membership_claimed: false },
      realization: { terminal: "InvitationDelivered", membership_claimed: false },
      admission: { disposition: "admitted" },
    });
    await expectProcessSuccess(running);
  } finally {
    if (running.exitCode === null) running.kill();
  }
});

function rendezvousCode(child) {
  let output = "";
  return new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error(`rendezvous code was not ready\n${output}`)), 10_000);
    const inspect = (chunk) => {
      output += chunk.toString();
      const match = output.match(/Rendezvous code: (C1-WS-[0-9A-F-]+)/);
      if (match) {
        clearTimeout(timeout);
        resolve(match[1]);
      }
    };
    child.stdout.on("data", inspect);
    child.stderr.on("data", inspect);
    child.once("exit", (status) => {
      clearTimeout(timeout);
      reject(new Error(`running host exited before producing a code (${status})\n${output}`));
    });
  });
}

function expectProcessSuccess(child) {
  if (child.exitCode !== null) return Promise.resolve(expect(child.exitCode).toBe(0));
  return new Promise((resolve, reject) => child.once("exit", (status) => {
    try {
      expect(status).toBe(0);
      resolve();
    } catch (error) {
      reject(error);
    }
  }));
}
