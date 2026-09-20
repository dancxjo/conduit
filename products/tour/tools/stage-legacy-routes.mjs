import { mkdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

// Only these already-published routes retain compatibility. No Book or Tour
// application owns public route truth, and redirects never choose a destination
// from caller input. The legacy Tour remains available through its explicit
// repository-development entrance.
const routes = ["", "a-form-you-can-run", "fronts-backs-and-implementation", "hosts-make-forms-real", "one-form-across-several-hosts", "the-body-one-computer-one-machine-or-many", "many-forms-one-body-wide-realization", "birth-spores-and-the-creche", "meet-one-gear", "same-front-different-implementation"];

async function stageRedirects(root, entrance) {
  for (const route of routes) {
    const target = route ? "../../workspace/" : "../workspace/";
    const directory = resolve(root, entrance, route);
    await mkdir(directory, { recursive: true });
    await writeFile(resolve(directory, "index.html"), `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <title>Redirecting to your body</title>
  <script>location.replace(${JSON.stringify(target)} + location.search + location.hash);</script>
  <meta http-equiv="refresh" content="0; url=${target}">
</head>
<body><p>Redirecting to <a href="${target}">your body</a>.</p></body>
</html>
`);
  }
}

export async function stageLegacyTourRoutes(root) {
  await stageRedirects(root, "tour");
  await stageRedirects(root, "book");
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  if (process.argv.length !== 3) throw new Error("usage: stage-legacy-routes.mjs PAGES_ROOT");
  await stageLegacyTourRoutes(process.argv[2]);
}
