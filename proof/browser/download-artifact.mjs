import { readFile, writeFile } from "node:fs/promises";

export async function downloadArtifact(page, control, { retainAt } = {}) {
  await page.evaluate(() => Object.defineProperty(globalThis, "showSaveFilePicker", {
    configurable: true,
    value: undefined,
  }));
  const pending = page.waitForEvent("download");
  await control.click();
  const download = await pending;
  const path = await download.path();
  if (!path) throw new Error("browser artifact handoff produced no downloadable file");
  const bytes = new Uint8Array(await readFile(path));
  if (retainAt !== undefined) {
    if (typeof retainAt !== "string" || retainAt.length < 1 || retainAt.length > 4096) {
      throw new TypeError("browser artifact retention requires one bounded explicit path");
    }
    await writeFile(retainAt, bytes, { flag: "wx" });
  }
  return Object.freeze({
    filename: download.suggestedFilename(),
    bytes,
  });
}

export async function sha256(bytes) {
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", bytes));
  return `sha256:${Array.from(digest, (byte) => byte.toString(16).padStart(2, "0")).join("")}`;
}
