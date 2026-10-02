import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";

const repository = fileURLToPath(new URL("../..", import.meta.url));
const directory = path.join(repository, "target/journeys/field-station-clock");
const digest = bytes => createHash("sha256").update(bytes).digest("hex");

export async function beginFieldStationJourney() {
  const sourceCommit = process.env.CONDUIT_CHECKOUT_SHA
    ?? execFileSync("git", ["rev-parse", "HEAD"], { cwd: repository, encoding: "utf8" }).trim();
  if (!/^[0-9a-f]{40}$/.test(sourceCommit)) throw new Error("Field Station capture requires an exact source commit");
  await rm(directory, { recursive: true, force: true });
  await mkdir(directory, { recursive: true });
  const steps = [];
  return {
    async capture(page, { id, title, action, observation }) {
      const screenshot = `${steps.length + 1}-${id}.png`;
      await page.screenshot({ path: path.join(directory, screenshot), fullPage: true });
      steps.push({ id, title, action, observation, screenshot,
        sha256: digest(await readFile(path.join(directory, screenshot))) });
    },
    async finish(observed) {
      const evidencePath = "runtime-evidence.json";
      const evidenceBytes = `${JSON.stringify(observed, null, 2)}\n`;
      await writeFile(path.join(directory, evidencePath), evidenceBytes);
      const manifest = {
        schema: "conduit.user-journey/v1",
        title: "Keep a Field Station clock through a reload",
        summary: "Open a clock, reload its page while preserving its Body, then lull it through the visible control.",
        sourceCommit,
        environment: "Chromium",
        steps,
        evidence: { path: evidencePath, sha256: digest(evidenceBytes) },
      };
      await writeFile(path.join(directory, "index.json"), `${JSON.stringify(manifest, null, 2)}\n`);
    },
  };
}
