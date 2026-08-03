import { createReadStream } from "node:fs";
import { stat } from "node:fs/promises";
import { createServer } from "node:http";
import { extname, resolve, sep } from "node:path";

const sourceRoot = resolve(".");
const artifactRoot = process.env.CONDUIT_TOUR_SITE
  ? resolve(process.env.CONDUIT_TOUR_SITE)
  : undefined;
const mediaTypes = new Map([
  [".html", "text/html; charset=utf-8"],
  [".css", "text/css; charset=utf-8"],
  [".js", "text/javascript; charset=utf-8"],
  [".mjs", "text/javascript; charset=utf-8"],
  [".json", "application/json; charset=utf-8"],
  [".panel", "text/plain; charset=utf-8"],
  [".wasm", "application/wasm"],
]);

const requestedPort = parseInt(process.argv[2] ?? process.env.PORT ?? "4173", 10);
const requestedHost = process.env.CONDUIT_STATIC_HOST ?? "127.0.0.1";
const landingPath = process.env.CONDUIT_STATIC_LANDING;

const server = createServer(async (request, response) => {
  try {
    const pathname = decodeURIComponent(new URL(request.url, "http://127.0.0.1").pathname);
    if (pathname === "/" && landingPath) {
      response.writeHead(302, { Location: landingPath }).end();
      return;
    }
    const roots = artifactRoot ? [artifactRoot, sourceRoot] : [sourceRoot];
    let file;
    let metadata;
    for (const root of roots) {
      const candidate = resolve(root, `.${pathname}`);
      if (candidate !== root && !candidate.startsWith(`${root}${sep}`)) {
        continue;
      }
      try {
        const candidateMetadata = await stat(candidate);
        if (candidateMetadata.isFile()) {
          file = candidate;
          metadata = candidateMetadata;
          break;
        }
      } catch {
        // The test harness remains source-owned while release paths come from
        // the assembled artifact whenever it contains the requested file.
      }
    }
    if (!file || !metadata) {
      response.writeHead(404).end();
      return;
    }
    response.writeHead(200, {
      "Content-Length": metadata.size,
      "Content-Type": mediaTypes.get(extname(file)) ?? "application/octet-stream",
      "Cache-Control": "no-store",
    });
    createReadStream(file).pipe(response);
  } catch {
    response.writeHead(404).end();
  }
});

server.listen(requestedPort, requestedHost, () => {
  const address = server.address();
  const actualPort = typeof address === "object" && address ? address.port : requestedPort;
  if (landingPath) {
    console.log(`WORKBENCH_URL=http://${requestedHost}:${actualPort}${landingPath}`);
  } else {
    console.log(`READY:${actualPort}`);
  }
});

for (const signal of ["SIGINT", "SIGTERM"]) {
  process.on(signal, () => server.close(() => process.exit(0)));
}
