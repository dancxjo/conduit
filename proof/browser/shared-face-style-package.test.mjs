import assert from "node:assert/strict";
import test from "node:test";
import { createHash } from "node:crypto";
import { mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const templates = [
  "targets/browser/patchbay-workbench/assets/patchbay.application.template.json",
  "targets/browser/workspace/workspace.application.template.json",
  "targets/browser/handbook/handbook.application.template.json",
  "targets/browser/examples/clock-lab/application.template.json",
];
const css = await readFile("targets/browser/host/assets/conduit.css");
const producer = path.resolve("targets/browser/tools/build-browser-application-package.mjs");

test("every shared Face stylesheet consumer admits the current bytes through the package producer", async () => {
  const scratch = await mkdtemp(path.join(tmpdir(), "conduit-shared-face-style-"));
  try {
    for (const [index, templatePath] of templates.entries()) {
      const original = JSON.parse(await readFile(templatePath, "utf8"));
      const styles = original.resources.filter(resource => resource.kind === "style" &&
        (resource.path === "conduit.css" || resource.source?.endsWith("/conduit.css")));
      assert.equal(styles.length, 1, `${templatePath}: one shared stylesheet`);
      const resource = styles[0];
      assert.ok(css.length > 0 && css.length <= resource.maximum_bytes,
        `${templatePath}: ${css.length} stylesheet bytes exceed declared ${resource.maximum_bytes}`);
      const destination = path.join(scratch, String(index));
      const stylePath = path.join(destination, resource.path);
      await mkdir(path.dirname(stylePath), { recursive: true });
      await writeFile(stylePath, css);
      // Exercise the generic producer with each real consumer's admitted style
      // entry; this is a style-boundary proof, not a staged application claim.
      const template = { ...original, resources: [resource] };
      const input = path.join(destination, "template.json");
      await writeFile(input, JSON.stringify(template));
      const built = spawnSync(process.execPath, [producer, input, destination, "style.application.json"], { encoding: "utf8" });
      assert.equal(built.status, 0, `${templatePath}: ${built.stderr}`);
      const manifest = JSON.parse(await readFile(path.join(destination, "style.application.json"), "utf8"));
      assert.equal(manifest.resources[0].maximum_bytes, resource.maximum_bytes);
      assert.equal(manifest.resources[0].sha256, `sha256:${createHash("sha256").update(css).digest("hex")}`);
      assert.deepEqual(await readFile(stylePath), css);
      template.resources = [{ ...resource, maximum_bytes: css.length - 1 }];
      await writeFile(input, JSON.stringify(template));
      const refused = spawnSync(process.execPath, [producer, input, destination, "refused.application.json"], { encoding: "utf8" });
      assert.notEqual(refused.status, 0, `${templatePath}: undersized declarations must refuse`);
      assert.match(refused.stderr, /application resource exceeds bound/);
    }
  } finally {
    await rm(scratch, { recursive: true, force: true });
  }
});

// Component Looks belong to the component shadow tree. A style resource is
// activated in document.head by the loader and would leak its generic rules.
test("owner Face Looks remain admitted assets without document-wide activation", async () => {
  for (const templatePath of templates.slice(1, 3)) {
    const manifest = JSON.parse(await readFile(templatePath, "utf8"));
    const look = manifest.resources.find(resource => resource.role === "owner-face-look");
    assert.ok(look, `${templatePath}: component Look is retained`);
    assert.equal(look.kind, "content", `${templatePath}: shadow Look must not activate globally`);
    assert.equal(look.path, "owner-face-look.css");
  }
});

// Native admission wire profiles grew this module beyond its former 12 KiB
// envelope. Exercise its real package boundary without staging a full product.
test("workspace admits the native image module and refuses an undersized envelope", async () => {
  const original = JSON.parse(await readFile(templates[1], "utf8"));
  const resource = original.resources.find(resource => resource.role === "conduitos-image");
  const bytes = await readFile("targets/conduitos/deployment/browser/image.mjs");
  const scratch = await mkdtemp(path.join(tmpdir(), "conduit-native-image-package-"));
  try {
    const staged = path.join(scratch, resource.path);
    await mkdir(path.dirname(staged), { recursive: true });
    await writeFile(staged, bytes);
    const input = path.join(scratch, "template.json");
    for (const [maximum, accepted] of [[resource.maximum_bytes, true], [bytes.length - 1, false]]) {
      await writeFile(input, JSON.stringify({ ...original, resources: [{ ...resource, maximum_bytes: maximum }] }));
      const built = spawnSync(process.execPath, [producer, input, scratch, "image.application.json"], { encoding: "utf8" });
      assert.equal(built.status === 0, accepted, built.stderr);
      if (accepted) {
        const manifest = JSON.parse(await readFile(path.join(scratch, "image.application.json"), "utf8"));
        assert.equal(manifest.resources[0].sha256, `sha256:${createHash("sha256").update(bytes).digest("hex")}`);
      } else {
        assert.match(built.stderr, /application resource exceeds bound/);
      }
    }
  } finally {
    await rm(scratch, { recursive: true, force: true });
  }
});
