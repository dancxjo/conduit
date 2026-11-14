import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { webcrypto } from "node:crypto";
import test from "node:test";
import { buildBrowserBundleImage } from "../../../../deployment/browser/browser-bundle.mjs";

globalThis.crypto ??= webcrypto;

test("Rust differential fixture matches the existing JS BrowserBundle producer", async () => {
  const fixture = JSON.parse(await readFile(new URL("./js-browser-bundle.json", import.meta.url), "utf8"));
  const retained = new Map(fixture.payloads.map(({ path, bytes }) => [path, new Uint8Array(bytes)]));
  const distribution = {
    manifest: fixture.distribution,
    payloads: fixture.distribution.files.map(file => ({ ...file, bytes: retained.get(file.path) })),
  };
  const release = await buildBrowserBundleImage({ checked: fixture.checked, distribution });
  assert.deepEqual(release.manifest, fixture.release);
  assert.deepEqual(release.payloads.map(({ path, bytes }) => ({ path, bytes: [...bytes] })), fixture.payloads);
});
