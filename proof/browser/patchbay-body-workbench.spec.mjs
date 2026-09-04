import { spawn } from "node:child_process";
import { expect, test } from "@playwright/test";

let server;
let hostedTruth;

async function openWorkbench(page, entrance) {
  server = spawn("target/debug/patchbay-html", ["--body-workbench-fixture", entrance], {
    cwd: new URL("../..", import.meta.url).pathname,
    stdio: ["ignore", "pipe", "pipe"],
  });
  let output = "";
  const url = await new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error(`Body workbench was not ready\n${output}`)), 10_000);
    const inspect = (chunk) => {
      output += chunk.toString();
      const match = output.match(/PATCHBAY_HTML_URL=(http:\/\/127\.0\.0\.1:\d+)/);
      if (match) { clearTimeout(timeout); resolve(match[1]); }
    };
    server.stdout.on("data", inspect);
    server.stderr.on("data", inspect);
    server.once("exit", code => reject(new Error(`Body workbench exited (${code})\n${output}`)));
  });
  await page.goto(url);
  await expect(page.locator('[data-application-key="product-status"]')).toContainText("Presentation revision");
  return url;
}

test.afterEach(() => server?.kill());

for (const entrance of ["hosted", "external"]) {
  test(`${entrance} graduated Body opens in the same semantic workbench`, async ({ page }) => {
    await openWorkbench(page, entrance);
    const snapshot = await page.request.get(new URL("/api/snapshot", page.url()).href).then(response => response.json());
    const originalEvidence = snapshot.body_workbench.encoded_evidence;
    const attachment = snapshot.body_workbench.entrance;
    if (entrance === "hosted") {
      expect(attachment).toEqual({
        kind: "hosted",
        plan_id: "plan/roseau-hosted-patchbay",
        implementation_id: "browser/patchbay-surface@1",
      });
    } else {
      expect(attachment).toEqual({ kind: "external-reader" });
    }

    const current = snapshot.body_workbench.current;
    const history = snapshot.body_workbench.history;
    const projectedTruth = {
      body_id: snapshot.body_workbench.body_id,
      program: current.program,
      lifecycle: current.lifecycle,
      current_hosts: current.current_hosts,
      biography_identity: history.entries.map(entry => ({
        moment: entry.moment,
        sign_id: entry.exact.record.sign_id,
      })),
    };
    if (entrance === "hosted") {
      hostedTruth = projectedTruth;
    } else {
      expect(projectedTruth).toEqual(hostedTruth);
    }
    expect(JSON.stringify(history)).not.toMatch(/timestamp|wall.?clock|utc/i);
    expect(history.entries.every(entry => entry.linear.includes(snapshot.body_workbench.body_id))).toBe(true);

    await expect(page.getByRole("heading", { name: "Roseau" })).toBeVisible();
    await expect(page.locator("#body-workbench-status")).toContainText("Lulled · 1 Part · 1 current Host");
    await expect(page.locator("#body-workbench-placement")).toContainText(
      entrance === "hosted" ? "hosted by this Body" : "external Patchbay",
    );
    await expect(page.locator("#body-workbench-current")).toContainText("Morse relay");
    await expect(page.getByRole("button", { name: "Wake", exact: true })).toBeVisible();
    const workbenchNavigation = page.getByRole("navigation", { name: "Body workbench" });
    await expect(workbenchNavigation.getByRole("button"))
      .toHaveText(["Program", "Body", "History"]);

    const historyButton = workbenchNavigation.getByRole("button", { name: "History", exact: true });
    if (entrance === "external") {
      await historyButton.focus();
      await historyButton.press("Enter");
    } else {
      await historyButton.click();
    }
    await expect(page.getByRole("heading", { name: "What has happened to it?" })).toBeVisible();
    await expect(page.locator('#body-history [data-application-component="artifact"]')).toHaveCount(4);
    await expect(page.locator("#body-history")).toContainText("Graduated from the Crèche");
    await page.getByText("Linear BODY / SIGNS evidence", { exact: true }).click();
    await expect(page.locator('#body-linear [data-application-component="artifact"]')).toHaveCount(4);
    await expect(page.locator("body")).toHaveAttribute("data-place", "Body");
    await expect(page.locator("body")).toHaveAttribute("data-aspect", "Signs");

    await workbenchNavigation.getByRole("button", { name: "Program", exact: true }).click();
    await expect(page.locator("body")).toHaveAttribute("data-place", "Program");
    await expect(page.locator("body")).toHaveAttribute("data-aspect", "Structure");

    await workbenchNavigation.getByRole("button", { name: "Body", exact: true }).click();
    await page.locator("#body-workbench-current details summary").click();
    await expect(page.locator("#body-workbench-exact")).toContainText(snapshot.body_workbench.body_id);

    await page.getByRole("button", { name: "Wake", exact: true }).click();
    await expect(page.locator("#front-door-feedback")).toContainText(
      "wake Refused(OperationUnavailable)",
    );
    const afterAction = await page.request.get(new URL("/api/snapshot", page.url()).href).then(response => response.json());
    expect(afterAction.body_workbench.encoded_evidence).toEqual(originalEvidence);

    server.kill();
    await expect(page.getByRole("heading", { name: "Roseau" })).toBeVisible();
    expect(snapshot.body_workbench.encoded_evidence).toEqual(originalEvidence);
  });
}
