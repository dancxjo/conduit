# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: static-body-application.spec.mjs >> static Handbook applications retain independent local Bodies through use, navigation, exclusion, reset and browser restart
- Location: proof/browser/static-body-application.spec.mjs:53:1

# Error details

```
Error: Journey capture requires a clean source tree, including untracked files
```

# Test source

```ts
  1  | import { execFileSync } from "node:child_process";
  2  | import { createHash } from "node:crypto";
  3  | import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
  4  | import { fileURLToPath } from "node:url";
  5  | import path from "node:path";
  6  | 
  7  | const repository = fileURLToPath(new URL("../..", import.meta.url));
  8  | const directory = path.join(repository, "target/journeys/handbook");
  9  | const digest = bytes => createHash("sha256").update(bytes).digest("hex");
  10 | 
  11 | export async function beginHandbookJourney() {
  12 |   const sourceCommit = execFileSync("git", ["rev-parse", "HEAD"], { cwd: repository, encoding: "utf8" }).trim();
  13 |   if (!/^[0-9a-f]{40}$/.test(sourceCommit)
  14 |     || (process.env.CONDUIT_CHECKOUT_SHA && process.env.CONDUIT_CHECKOUT_SHA !== sourceCommit)) {
  15 |     throw new Error("Journey capture requires the actual checkout commit");
  16 |   }
  17 |   if (execFileSync("git", ["status", "--porcelain", "--untracked-files=all"], { cwd: repository, encoding: "utf8" }).trim()) {
> 18 |     throw new Error("Journey capture requires a clean source tree, including untracked files");
     |           ^ Error: Journey capture requires a clean source tree, including untracked files
  19 |   }
  20 |   await rm(directory, { recursive: true, force: true });
  21 |   await mkdir(directory, { recursive: true });
  22 |   const steps = [];
  23 |   return {
  24 |     async capture(page, { id, title, action, observation }) {
  25 |       const screenshot = `${steps.length + 1}-${id}.png`;
  26 |       await page.screenshot({ path: path.join(directory, screenshot) });
  27 |       steps.push({ id, title, action, observation, screenshot,
  28 |         sha256: digest(await readFile(path.join(directory, screenshot))) });
  29 |     },
  30 |     async finish(observed) {
  31 |       if (execFileSync("git", ["rev-parse", "HEAD"], { cwd: repository, encoding: "utf8" }).trim() !== sourceCommit
  32 |         || execFileSync("git", ["status", "--porcelain", "--untracked-files=all"], { cwd: repository, encoding: "utf8" }).trim()) {
  33 |         throw new Error("Journey source changed during capture; refusing to seal evidence");
  34 |       }
  35 |       const evidencePath = "runtime-evidence.json";
  36 |       const evidenceBytes = `${JSON.stringify(observed, null, 2)}\n`;
  37 |       await writeFile(path.join(directory, evidencePath), evidenceBytes);
  38 |       const manifest = {
  39 |         schema: "conduit.user-journey/v1",
  40 |         title: "Try a clock, inspect it, and keep your changes",
  41 |         summary: "Open your local Handbook, check and run a clock, inspect its resident Patchbay, edit its timing, then lull and recover the same Body.",
  42 |         sourceCommit,
  43 |         environment: "Chromium",
  44 |         steps,
  45 |         evidence: { path: evidencePath, sha256: digest(evidenceBytes) },
  46 |       };
  47 |       await writeFile(path.join(directory, "index.json"), `${JSON.stringify(manifest, null, 2)}\n`);
  48 |     },
  49 |   };
  50 | }
  51 | 
```