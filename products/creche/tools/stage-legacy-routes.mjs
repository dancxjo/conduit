import { mkdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

export async function stageLegacyCrecheRoute(root) {
  const directory = resolve(root, "creche");
  const target = "../workspace/";
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

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  if (process.argv.length !== 3) throw new Error("usage: stage-legacy-routes.mjs PAGES_ROOT");
  await stageLegacyCrecheRoute(process.argv[2]);
}
