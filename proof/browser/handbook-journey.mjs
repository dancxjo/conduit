import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";

const repository = fileURLToPath(new URL("../..", import.meta.url));
const directory = path.join(repository, "target/journeys/handbook");
const digest = bytes => createHash("sha256").update(bytes).digest("hex");

export async function beginHandbookJourney() {
  const sourceCommit = execFileSync("git", ["rev-parse", "HEAD"], { cwd: repository, encoding: "utf8" }).trim();
  if (!/^[0-9a-f]{40}$/.test(sourceCommit)
    || (process.env.CONDUIT_CHECKOUT_SHA && process.env.CONDUIT_CHECKOUT_SHA !== sourceCommit)) {
    throw new Error("Journey capture requires the actual checkout commit");
  }
  if (execFileSync("git", ["status", "--porcelain", "--untracked-files=all"], { cwd: repository, encoding: "utf8" }).trim()) {
    throw new Error("Journey capture requires a clean source tree, including untracked files");
  }
  await rm(directory, { recursive: true, force: true });
  await mkdir(directory, { recursive: true });
  const steps = [];
  return {
    async capture(page, { id, title, action, observation }) {
      const screenshot = `${steps.length + 1}-${id}.png`;
      await page.screenshot({ path: path.join(directory, screenshot) });
      steps.push({ id, title, action, observation, screenshot,
        sha256: digest(await readFile(path.join(directory, screenshot))) });
    },
    async finish(observed) {
      if (execFileSync("git", ["rev-parse", "HEAD"], { cwd: repository, encoding: "utf8" }).trim() !== sourceCommit
        || execFileSync("git", ["status", "--porcelain", "--untracked-files=all"], { cwd: repository, encoding: "utf8" }).trim()) {
        throw new Error("Journey source changed during capture; refusing to seal evidence");
      }
      const evidencePath = "runtime-evidence.json";
      const evidenceBytes = `${JSON.stringify(observed, null, 2)}\n`;
      await writeFile(path.join(directory, evidencePath), evidenceBytes);
      const manifest = {
        schema: "conduit.user-journey/v1",
        title: "Try a clock, inspect it, and keep your changes",
        summary: "Open your local Handbook, check and run a clock, inspect its resident Patchbay, edit its timing, then lull and recover the same Body.",
        sourceCommit,
        environment: "Chromium",
        steps,
        evidence: { path: evidencePath, sha256: digest(evidenceBytes) },
      };
      await writeFile(path.join(directory, "index.json"), `${JSON.stringify(manifest, null, 2)}\n`);
    },
  };
}
