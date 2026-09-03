import { readFile } from "node:fs/promises";

export async function downloadArtifact(page, control) {
  await page.evaluate(() => Object.defineProperty(globalThis, "showSaveFilePicker", {
    configurable: true,
    value: undefined,
  }));
  const pending = page.waitForEvent("download");
  await control.click();
  const download = await pending;
  const path = await download.path();
  if (!path) throw new Error("browser artifact handoff produced no downloadable file");
  return Object.freeze({
    filename: download.suggestedFilename(),
    bytes: new Uint8Array(await readFile(path)),
  });
}

export async function sha256(bytes) {
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", bytes));
  return `sha256:${Array.from(digest, (byte) => byte.toString(16).padStart(2, "0")).join("")}`;
}
