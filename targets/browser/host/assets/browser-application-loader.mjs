import { openBrowserApplicationStorage } from "./browser-application-storage.mjs";
import { createApplicationPresentationHost } from "./application-presentation.mjs";

const PACKAGE_SCHEMA = "conduit.browser/application-package@1";
const MAXIMUM_PACKAGE_BYTES = 32 * 1024;
const MAXIMUM_RESOURCES = 64;
const MAXIMUM_DEPENDENCIES = 16;
const MAXIMUM_HOST_IMPLEMENTATIONS = 16;
const MAXIMUM_RESOURCE_BYTES = 16 * 1024 * 1024;
const MAXIMUM_TOTAL_RESOURCE_BYTES = 32 * 1024 * 1024;
const RESOURCE_KINDS = new Set(["module", "classic-script", "style", "content", "wasm"]);
const DIGEST_PATTERN = /^sha256:[0-9a-f]{64}$/;
const encoder = new TextEncoder();
const decoder = new TextDecoder("utf-8", { fatal: true });

function boundedText(value, label, maximum = 160) {
  if (typeof value !== "string" || value.length === 0 || encoder.encode(value).length > maximum || /[\0\n,=]/.test(value)) {
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

async function sha256(bytes) {
  const digest = await crypto.subtle.digest("SHA-256", bytes);
  return `sha256:${Array.from(new Uint8Array(digest), (byte) => byte.toString(16).padStart(2, "0")).join("")}`;
}

async function boundedFetch(url, maximumBytes, label) {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`${label} is unavailable`);
  const declared = Number(response.headers.get("content-length"));
  if (Number.isFinite(declared) && declared > maximumBytes) throw new Error(`${label} exceeds its admitted bound`);
  const bytes = new Uint8Array(await response.arrayBuffer());
  if (bytes.length === 0 || bytes.length > maximumBytes) throw new Error(`${label} exceeds its admitted bound`);
  return bytes;
}

function canonicalPackage(manifest) {
  const lines = [
    "conduit.browser/application-package-content@1",
    `application\0${manifest.applicationId}`,
    `state\0${manifest.stateCompatibility.identity}\0${manifest.stateCompatibility.version}`,
  ];
  for (const implementation of manifest.hostImplementations) lines.push(`host-implementation\0${implementation}`);
  for (const resource of manifest.resources) {
    const dependencies = resource.dependencies.map(({ role, specifier }) => `${role}=${specifier}`).join(",");
    lines.push(`resource\0${resource.role}\0${resource.kind}\0${resource.path}\0${resource.maximumBytes}\0${resource.sha256}\0${dependencies}`);
  }
  return encoder.encode(`${lines.join("\n")}\n`);
}

function admitManifest(document, manifestUrl) {
  if (!document || document.schema !== PACKAGE_SCHEMA) throw new Error("browser application package schema is unsupported");
  const applicationId = boundedText(document.application_id, "application identity");
  if (!DIGEST_PATTERN.test(document.package_digest ?? "")) throw new Error("application package identity is invalid");
  const stateCompatibility = Object.freeze({
    identity: boundedText(document.state_compatibility?.identity, "application state compatibility identity"),
    version: document.state_compatibility?.version,
  });
  if (!Number.isSafeInteger(stateCompatibility.version) || stateCompatibility.version < 1) {
    throw new Error("application state compatibility version is invalid");
  }
  if (!Array.isArray(document.host_implementations) || document.host_implementations.length === 0
    || document.host_implementations.length > MAXIMUM_HOST_IMPLEMENTATIONS) {
    throw new Error("browser application Host implementation selection is outside its admitted bound");
  }
  const hostImplementations = Object.freeze([...new Set(document.host_implementations.map((identity) =>
    boundedText(identity, "browser application Host implementation identity")))].sort());
  if (hostImplementations.length !== document.host_implementations.length) {
    throw new Error("browser application Host implementation selection is duplicated");
  }
  if (!Array.isArray(document.resources) || document.resources.length === 0 || document.resources.length > MAXIMUM_RESOURCES) {
    throw new Error("application package resource count is outside its admitted bound");
  }
  const packageRoot = new URL(".", manifestUrl);
  const seenRoles = new Set();
  const seenUrls = new Set();
  let totalMaximumBytes = 0;
  const resources = document.resources.map((resource) => {
    const role = boundedText(resource?.role, "application resource role", 64);
    const kind = boundedText(resource?.kind, "application resource kind", 32);
    if (!RESOURCE_KINDS.has(kind)) throw new Error("application resource kind is unsupported");
    if (seenRoles.has(role)) throw new Error("application resource role is duplicated");
    seenRoles.add(role);
    const url = packageUrl(resource.path, packageRoot, "application resource path");
    if (seenUrls.has(url.href)) throw new Error("application resource path is duplicated");
    seenUrls.add(url.href);
    if (!Number.isSafeInteger(resource.maximum_bytes) || resource.maximum_bytes < 1 || resource.maximum_bytes > MAXIMUM_RESOURCE_BYTES) {
      throw new Error("application resource byte bound is invalid");
    }
    if (!DIGEST_PATTERN.test(resource.sha256 ?? "")) throw new Error("application resource digest is invalid");
    if (!Array.isArray(resource.dependencies) || resource.dependencies.length > MAXIMUM_DEPENDENCIES) {
      throw new Error("application resource dependency count is outside its admitted bound");
    }
    const dependencyRoles = new Set();
    const dependencies = resource.dependencies.map((dependency) => {
      const dependencyRole = boundedText(dependency?.role, "application dependency role", 64);
      const specifier = boundedText(dependency?.specifier, "application dependency specifier", 256);
      if (dependencyRoles.has(dependencyRole)) throw new Error("application dependency role is duplicated");
      dependencyRoles.add(dependencyRole);
      return Object.freeze({ role: dependencyRole, specifier });
    });
    totalMaximumBytes += resource.maximum_bytes;
    return Object.freeze({ role, kind, path: resource.path, url, maximumBytes: resource.maximum_bytes, sha256: resource.sha256, dependencies: Object.freeze(dependencies) });
  });
  if (totalMaximumBytes > MAXIMUM_TOTAL_RESOURCE_BYTES) throw new Error("application package total byte bound is exceeded");
  if (!seenRoles.has("application-module") || !seenRoles.has("runtime")) throw new Error("application package has no module or runtime");
  for (const resource of resources) {
    for (const dependency of resource.dependencies) {
      if (!seenRoles.has(dependency.role) || dependency.role === resource.role) throw new Error("application resource dependency is invalid");
    }
  }
  return Object.freeze({ schema: PACKAGE_SCHEMA, applicationId, packageDigest: document.package_digest, stateCompatibility, hostImplementations, resources: Object.freeze(resources) });
}

async function executeClassic(resource, bytes) {
  const url = URL.createObjectURL(new Blob([bytes], { type: "text/javascript" }));
  try {
    await new Promise((resolve, reject) => {
      const script = document.createElement("script");
      script.src = url;
      script.dataset.applicationResource = resource.role;
      script.addEventListener("load", resolve, { once: true });
      script.addEventListener("error", () => reject(new Error(`application script ${resource.role} refused`)), { once: true });
      document.head.append(script);
    });
  } finally { URL.revokeObjectURL(url); }
}

async function loadProfileGatedBootModule(bytes, expectedDigest) {
  if (!(bytes instanceof Uint8Array) || bytes.byteLength < 1 || bytes.byteLength > 64 * 1024
    || !DIGEST_PATTERN.test(expectedDigest ?? "") || await sha256(bytes) !== expectedDigest) {
    throw new Error("profile-gated Boot module failed exact byte admission");
  }
  const source = decoder.decode(bytes);
  if (/\bimport\s*(?:\(|["'])|\bfrom\s*["']/.test(source)) {
    throw new Error("profile-gated Boot module is not self-contained");
  }
  const url = URL.createObjectURL(new Blob([source], { type: "text/javascript" }));
  let module;
  try { module = await import(url); } finally { URL.revokeObjectURL(url); }
  if (typeof module.admitBrowserBoot !== "function" || typeof module.observeBrowserHostEnvironment !== "function"
    || !Array.isArray(module.BROWSER_IMPLEMENTATION_CATALOG)) {
    throw new Error("profile-gated Boot module has an incompatible finite entrance");
  }
  return Object.freeze({
    admitBrowserBoot: module.admitBrowserBoot,
    observeBrowserHostEnvironment: module.observeBrowserHostEnvironment,
    implementationCatalog: Object.freeze(module.BROWSER_IMPLEMENTATION_CATALOG.map(({ implementation_id, revision }) => Object.freeze({ implementation_id, revision }))),
  });
}

async function admitProfileGatedBrowserBoot({
  moduleBytes, moduleDigest, imageBytes, expectedImageId, expectedProfileId,
  runtimeBytes, artifactContentDigest, bootId,
}) {
  const module = await loadProfileGatedBootModule(moduleBytes, moduleDigest);
  return module.admitBrowserBoot({
    imageBytes,
    expectedImageId,
    expectedProfileId,
    runtimeBytes,
    bootModuleDigest: moduleDigest,
    artifactContentDigest,
    bootId,
    availableImplementations: module.implementationCatalog.map(({ implementation_id, revision }) => ({ id: implementation_id, revision })),
    observations: await module.observeBrowserHostEnvironment(globalThis),
    bundleVariant: "superset",
  });
}

export async function loadBrowserApplication(manifestReference) {
  const manifestUrl = new URL(manifestReference, document.baseURI);
  if (manifestUrl.origin !== location.origin) throw new Error("application package must be same-origin");
  const manifestBytes = await boundedFetch(manifestUrl, MAXIMUM_PACKAGE_BYTES, "application package manifest");
  let manifestDocument;
  try { manifestDocument = JSON.parse(decoder.decode(manifestBytes)); }
  catch { throw new Error("application package manifest is malformed"); }
  const manifest = admitManifest(manifestDocument, manifestUrl);
  const admittedBytes = new Map();
  const admittedResources = await Promise.all(manifest.resources.map(async (resource) => {
    const bytes = await boundedFetch(resource.url, resource.maximumBytes, `application resource ${resource.role}`);
    if (await sha256(bytes) !== resource.sha256) throw new Error(`application resource ${resource.role} changed identity`);
    return [resource.role, bytes];
  }));
  for (const [role, bytes] of admittedResources) admittedBytes.set(role, bytes);
  if (await sha256(canonicalPackage(manifest)) !== manifest.packageDigest) throw new Error("application package identity changed");

  const byRole = new Map(manifest.resources.map((resource) => [resource.role, resource]));
  const executed = new Set();
  const executing = new Set();
  async function executeClassicRole(role) {
    if (executed.has(role)) return;
    if (executing.has(role)) throw new Error("application script dependency cycle refused");
    const resource = byRole.get(role);
    if (resource.kind !== "classic-script") throw new Error("classic application dependency has the wrong kind");
    executing.add(role);
    for (const dependency of resource.dependencies) {
      const target = byRole.get(dependency.role);
      if (target.kind === "classic-script") await executeClassicRole(target.role);
    }
    await executeClassic(resource, admittedBytes.get(role));
    executing.delete(role);
    executed.add(role);
  }
  for (const resource of manifest.resources) {
    if (resource.kind === "classic-script") await executeClassicRole(resource.role);
    if (resource.kind === "style") {
      const style = document.createElement("style");
      style.dataset.applicationResource = resource.role;
      style.textContent = decoder.decode(admittedBytes.get(resource.role));
      document.head.append(style);
    }
  }

  const moduleUrls = new Map();
  const materializing = new Set();
  async function materializeModule(role) {
    if (moduleUrls.has(role)) return moduleUrls.get(role);
    if (materializing.has(role)) throw new Error("application module dependency cycle refused");
    const resource = byRole.get(role);
    if (resource?.kind !== "module") throw new Error("application module dependency has the wrong kind");
    materializing.add(role);
    let source = decoder.decode(admittedBytes.get(role));
    for (const dependency of resource.dependencies) {
      const target = byRole.get(dependency.role);
      if (target.kind !== "module") continue;
      const targetUrl = await materializeModule(target.role);
      const marker = JSON.stringify(dependency.specifier);
      if (!source.includes(marker)) throw new Error(`application module ${role} did not use its declared dependency`);
      source = source.split(marker).join(JSON.stringify(targetUrl));
    }
    source = source.split("import.meta.url").join(JSON.stringify(resource.url.href));
    if (/\bimport\s*\(/.test(source)) throw new Error(`application module ${role} uses dynamic import`);
    if (/(?:\bfrom\s*|\bimport\s*)["'](?!blob:)/.test(source)) {
      throw new Error(`application module ${role} uses an undeclared module`);
    }
    materializing.delete(role);
    const url = URL.createObjectURL(new Blob([source], { type: "text/javascript" }));
    moduleUrls.set(role, url);
    return url;
  }

  const storage = await openBrowserApplicationStorage(
    manifest.stateCompatibility.identity,
    manifest.stateCompatibility.version,
    manifest.packageDigest,
    { implementationRegistry: manifest.hostImplementations },
  );
  const bytes = (role) => admittedBytes.get(role)?.slice() ?? null;
  const text = (role) => {
    const resource = byRole.get(role);
    if (resource?.kind !== "content") throw new Error(`application content ${role} was not admitted`);
    return decoder.decode(admittedBytes.get(role));
  };
  const entryUrl = await materializeModule("application-module");
  let module;
  try { module = await import(entryUrl); }
  finally { for (const url of moduleUrls.values()) URL.revokeObjectURL(url); }
  if (typeof module.startApplication !== "function") throw new Error("application module has no bounded start entrance");
  const presentation = createApplicationPresentationHost();
  const presentationFor = (scope) => createApplicationPresentationHost(scope);
  const context = Object.freeze({
    schema: "conduit.browser/application-context@1", manifest, storage, presentation, presentationFor, bytes, text,
    admitProfileGatedBrowserBoot,
  });
  await module.startApplication(context);
  globalThis.__conduitBrowserApplication = context;
  return context;
}

const manifest = document.querySelector('meta[name="conduit-application-package"]')?.content.trim();
if (!manifest) throw new Error("browser application package is not declared");
loadBrowserApplication(manifest).catch((error) => {
  let status = document.querySelector("#host-state");
  if (!status) {
    const masthead = document.querySelector('[data-application-slot="product-masthead"]');
    if (masthead) {
      status = document.createElement("output");
      status.id = "host-state";
      status.setAttribute("role", "status");
      masthead.replaceChildren(status);
    }
  }
  const target = document.querySelector("#chapter") ?? document.body;
  if (status) status.textContent = "Browser application refused";
  target.replaceChildren(document.createTextNode(error instanceof Error ? error.message : String(error)));
  target.classList.add("error");
});
