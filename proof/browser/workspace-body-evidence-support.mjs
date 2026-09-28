import { writeFile } from "node:fs/promises";
import { join } from "node:path";
import { expect } from "@playwright/test";
import { startStaticProduct } from "./static-product-server.mjs";

/** Birth one canonical Workspace Body and retain its exact evidence for later Hosts. */
export async function birthWorkspaceEvidence(page, temporary, friendlyName, titles, filename) {
  const workspace = await startStaticProduct("target/workspace-product", "/conduit/workspace/");
  try {
    await page.goto(workspace.url);
    const birth = page.locator(".body-birth-runner");
    await birth.getByLabel("Friendly Body name", { exact: true }).fill(friendlyName);
    for (const checkbox of await birth.getByRole("checkbox").all()) {
      if (await checkbox.isChecked()) await checkbox.uncheck();
    }
    for (const title of titles) {
      await birth.getByRole("checkbox", { name: title, exact: true }).check();
    }
    await birth.getByRole("button", { name: "Birth Body", exact: true }).click();
    await expect(page.locator("[data-play-state]")).toHaveText("Lulled");
    const result = await page.evaluate(() => ({
      current: globalThis.__conduitWorkspace.current(),
      evidence: globalThis.__conduitWorkspace.evidence().evidence,
    }));
    const evidencePath = join(temporary, filename);
    await writeFile(evidencePath, `${JSON.stringify(result.evidence)}\n`, "utf8");
    return { ...result, evidencePath };
  } finally {
    workspace.child.kill();
  }
}
