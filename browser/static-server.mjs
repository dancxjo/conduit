import { createReadStream } from "node:fs";
import { stat } from "node:fs/promises";
import { createServer } from "node:http";
import { randomUUID, timingSafeEqual } from "node:crypto";
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
const testHostToken = process.env.CONDUIT_TEST_HOST_TOKEN;
const hostPolicyCookie = "conduit_test_host_policy";
const hostPolicyAdapter = "conduit.task-action-policy/0";
const maximumHostPolicyBytes = 8 * 1024;
const maximumHostPolicySessions = 128;
const hostPolicies = new Map();

function headerMatchesToken(value) {
  if (!testHostToken || typeof value !== "string") return false;
  const expected = Buffer.from(testHostToken);
  const actual = Buffer.from(value);
  return expected.length === actual.length && timingSafeEqual(expected, actual);
}

function cookieValue(request, name) {
  for (const field of (request.headers.cookie ?? "").split(";")) {
    const [key, ...parts] = field.trim().split("=");
    if (key === name) return decodeURIComponent(parts.join("="));
  }
  return null;
}

async function readBoundedJson(request) {
  const chunks = [];
  let bytes = 0;
  for await (const chunk of request) {
    bytes += chunk.length;
    if (bytes > maximumHostPolicyBytes) throw new Error("request-too-large");
    chunks.push(chunk);
  }
  return JSON.parse(Buffer.concat(chunks).toString("utf8"));
}

function retainHostPolicy(sessionId, policy) {
  hostPolicies.delete(sessionId);
  hostPolicies.set(sessionId, policy);
  while (hostPolicies.size > maximumHostPolicySessions) {
    hostPolicies.delete(hostPolicies.keys().next().value);
  }
}

const server = createServer(async (request, response) => {
  try {
    const pathname = decodeURIComponent(new URL(request.url, "http://127.0.0.1").pathname);
    if (pathname === "/__conduit-test/task-action-policy") {
      if (request.method !== "POST" ||
          !headerMatchesToken(request.headers["x-conduit-test-host-token"])) {
        response.writeHead(404).end();
        return;
      }
      const policy = await readBoundedJson(request);
      if (!policy || typeof policy !== "object" || Array.isArray(policy)) {
        response.writeHead(400).end();
        return;
      }
      const sessionId = cookieValue(request, hostPolicyCookie) ?? randomUUID();
      retainHostPolicy(sessionId, policy);
      response.writeHead(204, {
        "Cache-Control": "no-store",
        "Set-Cookie": `${hostPolicyCookie}=${encodeURIComponent(sessionId)}; HttpOnly; SameSite=Strict; Path=/; Max-Age=3600`,
      }).end();
      return;
    }
    if (pathname === "/__conduit/host-policy/task-action") {
      if (!testHostToken || request.method !== "GET") {
        response.writeHead(404).end();
        return;
      }
      const sessionId = cookieValue(request, hostPolicyCookie);
      const policy = sessionId ? hostPolicies.get(sessionId) : null;
      if (!policy) {
        response.writeHead(204, {
          "Cache-Control": "no-store",
          "X-Conduit-Host-Policy-Adapter": hostPolicyAdapter,
        }).end();
        return;
      }
      const body = JSON.stringify(policy);
      response.writeHead(200, {
        "Cache-Control": "no-store",
        "Content-Length": Buffer.byteLength(body),
        "Content-Type": "application/json; charset=utf-8",
        "X-Conduit-Host-Policy-Adapter": hostPolicyAdapter,
      }).end(body);
      return;
    }
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
