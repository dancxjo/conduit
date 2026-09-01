import { openBrowserApplicationStorage } from "./browser-application-storage.mjs";

const PACKAGE_SCHEMA = "conduit.browser/application-package@1";
const MAXIMUM_PACKAGE_BYTES = 16 * 1024;
const MAXIMUM_RESOURCES = 32;
const MAXIMUM_RESOURCE_BYTES = 16 * 1024 * 1024;
const MAXIMUM_TOTAL_RESOURCE_BYTES = 32 * 1024 * 1024;
const encoder = new TextEncoder();

function boundedText(value, label, maximum = 160) {
  if (typeof value !== "string" || value.length === 0 || encoder.encode(value).length > maximum) {
    throw new Error(`${label} is outside its admitted bound`);
  }
  return value;
}

function packageUrl(path, packageRoot, label) {
  boundedText(path, label, 256);
  const url = new URL(path, packageRoot);
  if (url.origin !== packageRoot.origin || !url.href.startsWith(packageRoot.href) || url.search || url.hash) {
    throw new Error(`${label} escapes the application package`);
  }
  return url;
}

async function boundedFetch(url, maximumBytes, label) {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`${label} is unavailable`);
  const declared = Number(response.headers.get("content-length"));
  if (Number.isFinite(declared) && declared > maximumBytes) throw new Error(`${label} exceeds its admitted bound`);
  const bytes = new Uint8Array(await response.arrayBuffer());
  if (bytes.length > maximumBytes) throw new Error(`${label} exceeds its admitted bound`);
  return bytes;
}

function admitManifest(document, manifestUrl) {
  if (!document || document.schema !== PACKAGE_SCHEMA) throw new Error("browser application package schema is unsupported");
  const identity = boundedText(document.identity, "application identity");
  if (!Number.isSafeInteger(document.version) || document.version < 1) throw new Error("application package version is invalid");
  if (!Array.isArray(document.resources) || document.resources.length === 0 || document.resources.length > MAXIMUM_RESOURCES) {
    throw new Error("application package resource count is outside its admitted bound");
  }
  const packageRoot = new URL("./", manifestUrl);
  const seenRoles = new Set();
  const seenUrls = new Set();
  let totalMaximumBytes = 0;
  const resources = document.resources.map((resource) => {
    const role = boundedText(resource?.role, "application resource role", 64);
    if (seenRoles.has(role)) throw new Error("application resource role is duplicated");
    seenRoles.add(role);
    const url = packageUrl(resource.path, packageRoot, "application resource path");
    if (seenUrls.has(url.href)) throw new Error("application resource path is duplicated");
    seenUrls.add(url.href);
    if (!Number.isSafeInteger(resource.maximum_bytes) || resource.maximum_bytes < 1 || resource.maximum_bytes > MAXIMUM_RESOURCE_BYTES) {
      throw new Error("application resource byte bound is invalid");
    }
    totalMaximumBytes += resource.maximum_bytes;
    return Object.freeze({ role, url, maximumBytes: resource.maximum_bytes });
  });
  if (totalMaximumBytes > MAXIMUM_TOTAL_RESOURCE_BYTES) throw new Error("application package total byte bound is exceeded");
  if (!seenRoles.has("application-module") || !seenRoles.has("runtime")) {
    throw new Error("application package has no module or runtime");
  }
  return Object.freeze({ schema: PACKAGE_SCHEMA, identity, version: document.version, resources: Object.freeze(resources) });
}

export async function loadBrowserApplication(manifestReference) {
  const manifestUrl = new URL(manifestReference, document.baseURI);
  if (manifestUrl.origin !== location.origin) throw new Error("application package must be same-origin");
  const manifestBytes = await boundedFetch(manifestUrl, MAXIMUM_PACKAGE_BYTES, "application package manifest");
  let manifestDocument;
  try { manifestDocument = JSON.parse(new TextDecoder().decode(manifestBytes)); }
  catch { throw new Error("application package manifest is malformed"); }
  const manifest = admitManifest(manifestDocument, manifestUrl);
  const admittedBytes = new Map();
  for (const resource of manifest.resources) {
    admittedBytes.set(resource.role, await boundedFetch(resource.url, resource.maximumBytes, `application resource ${resource.role}`));
  }
  const storage = await openBrowserApplicationStorage(manifest.identity, manifest.version);
  const resource = (role) => {
    const admitted = manifest.resources.find((candidate) => candidate.role === role);
    if (!admitted) throw new Error(`application resource ${role} was not admitted`);
    return admitted.url;
  };
  const module = await import(resource("application-module").href);
  if (typeof module.startApplication !== "function") throw new Error("application module has no bounded start entrance");
  const context = Object.freeze({
    schema: "conduit.browser/application-context@1",
    manifest,
    storage,
    resource,
    bytes: (role) => admittedBytes.get(role)?.slice() ?? null,
  });
  await module.startApplication(context);
  globalThis.__conduitBrowserApplication = context;
  return context;
}

const manifest = document.querySelector('meta[name="conduit-application-package"]')?.content.trim();
if (!manifest) throw new Error("browser application package is not declared");
loadBrowserApplication(manifest).catch((error) => {
  const status = document.querySelector("#host-state");
  const target = document.querySelector("#chapter") ?? document.body;
  if (status) status.textContent = "Browser application refused";
  target.replaceChildren(document.createTextNode(error instanceof Error ? error.message : String(error)));
  target.classList.add("error");
});
