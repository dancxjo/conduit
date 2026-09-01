import { createHash } from "node:crypto";
import { readFile, writeFile } from "node:fs/promises";
import { resolve, sep } from "node:path";

const [templatePath, destination] = process.argv.slice(2);
if (!templatePath || !destination) throw new Error("usage: build-browser-application-package TEMPLATE DESTINATION");

const template = JSON.parse(await readFile(templatePath, "utf8"));
if (template.schema !== "conduit.browser/application-package-template@1") throw new Error("unsupported application package template");
const root = resolve(destination) + sep;
const resources = [];
for (const resource of template.resources ?? []) {
  const path = resolve(destination, resource.path);
  if (!path.startsWith(root)) throw new Error(`application resource escapes destination: ${resource.path}`);
  const bytes = await readFile(path);
  if (bytes.length === 0 || bytes.length > resource.maximum_bytes) throw new Error(`application resource exceeds bound: ${resource.role}`);
  resources.push({
    role: resource.role,
    kind: resource.kind,
    path: resource.path,
    maximum_bytes: resource.maximum_bytes,
    sha256: `sha256:${createHash("sha256").update(bytes).digest("hex")}`,
    dependencies: resource.dependencies ?? [],
  });
}

const canonical = packageCanonical(template.application_id, template.state_compatibility, resources);
const manifest = {
  schema: "conduit.browser/application-package@1",
  application_id: template.application_id,
  package_digest: `sha256:${createHash("sha256").update(canonical).digest("hex")}`,
  state_compatibility: template.state_compatibility,
  resources,
};
await writeFile(resolve(destination, "book.application.json"), `${JSON.stringify(manifest, null, 2)}\n`);

function packageCanonical(applicationId, stateCompatibility, entries) {
  const lines = [
    "conduit.browser/application-package-content@1",
    `application\0${applicationId}`,
    `state\0${stateCompatibility.identity}\0${stateCompatibility.version}`,
  ];
  for (const entry of entries) {
    const dependencies = entry.dependencies.map(({ role, specifier }) => `${role}=${specifier}`).join(",");
    lines.push(`resource\0${entry.role}\0${entry.kind}\0${entry.path}\0${entry.maximum_bytes}\0${entry.sha256}\0${dependencies}`);
  }
  return `${lines.join("\n")}\n`;
}
