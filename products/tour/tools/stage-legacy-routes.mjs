import { mkdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

// Only these already-published routes retain compatibility. No Book application
// is staged, and the redirect never chooses a destination from caller input.
const routes = ["", "a-form-you-can-run", "faces-backs-and-implementation", "hosts-make-forms-real", "one-form-across-several-hosts", "the-body-one-computer-one-machine-or-many", "many-forms-one-body-wide-realization", "birth-spores-and-the-creche", "meet-one-gear", "same-face-different-implementation"];

export async function stageLegacyTourRoutes(root) {
  for (const route of routes) {
    const target = route ? `../../tour/${route}/` : "../tour/";
    const directory = resolve(root, "book", route);
    await mkdir(directory, { recursive: true });
    await writeFile(resolve(directory, "index.html"), `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <title>Redirecting to Tour</title>
  <script>location.replace(${JSON.stringify(target)} + location.search + location.hash);</script>
  <meta http-equiv="refresh" content="0; url=${target}">
</head>
<body><p>Redirecting to <a href="${target}">Tour</a>.</p></body>
</html>
`);
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  if (process.argv.length !== 3) throw new Error("usage: stage-legacy-routes.mjs PAGES_ROOT");
  await stageLegacyTourRoutes(process.argv[2]);
}
