import { applyStaticApplicationCsp } from "./static-application-csp.mjs";
import { prepareStaticBrowserBundle } from './static-browser-bundle.mjs';
import { readFile, writeFile, mkdir, lstat, readdir, copyFile, rename, rm } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

const repository = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../..');
const [application, release, destination, handbook] = process.argv.slice(2);
if (!application || !release || !destination || ![5, 6].includes(process.argv.length)) throw new Error('usage: package-static-application TEMPLATE RELEASE OUTPUT [HANDBOOK]');
const output = path.resolve(destination);
try { await lstat(output); throw new Error('Static application output already exists'); }
catch (error) { if (error.code !== 'ENOENT') throw error; }
const sourceRoot = path.dirname(path.resolve(application));
const template = JSON.parse(await readFile(application, 'utf8'));
if (template.schema !== 'conduit.browser/application-package-template@1'
  || !template.application_id || !template.state_compatibility?.identity
  || !Number.isSafeInteger(template.state_compatibility.version) || template.state_compatibility.version < 1
  || !Array.isArray(template.resources)) throw new Error('Invalid existing application package template');
if (template.loopback_owner_window !== undefined &&
  (template.loopback_owner_window !== true || template.application_id !== 'conduit.application/handbook')) {
  throw new Error('Only the reviewed Handbook may request a loopback Body owner window');
}
for (const role of ['application-module', 'birth-specification']) {
  if (template.resources.filter(resource => resource.role === role).length !== 1) throw new Error(`Required application resource: ${role}`);
}
if (template.resources.find(resource => resource.role === 'birth-specification').kind !== 'content') throw new Error('Birth specification must be admitted content');
const safe = value => typeof value === 'string' && value.length > 0 && value.length <= 256
  && !value.includes('\\') && !value.includes('%') && !path.isAbsolute(value)
  && value.split('/').every(part => part && part !== '.' && part !== '..');
async function regular(root, name) {
  if (!safe(name)) throw new Error(`Unsafe application path: ${name}`);
  let file = root;
  for (const part of name.split('/')) {
    file = path.join(file, part);
    if ((await lstat(file)).isSymbolicLink()) throw new Error('Application sources must not contain symlinks');
  }
  const stat = await lstat(file);
  if (!stat.isFile() || stat.size < 1 || stat.size > 16 * 1024 * 1024) throw new Error(`Invalid application resource: ${name}`);
  return file;
}
function run(script, ...args) {
  const result = spawnSync(process.execPath, [path.join(repository, script), ...args], { stdio: 'inherit', cwd: repository });
  if (result.error || result.status !== 0) throw new Error(`Package producer failed: ${script}`);
}
await mkdir(path.dirname(output), { recursive: true });
const staging = `${output}.staging-${process.pid}`;
await mkdir(staging);
try {
  if (handbook) {
    let files = 0, bytes = 0;
    async function copyDocumentary(directory, prefix = '') {
      for (const entry of await readdir(directory, { withFileTypes: true })) {
        const name = prefix + entry.name;
        if (entry.isDirectory()) await copyDocumentary(path.join(directory, entry.name), `${name}/`);
        else {
          const source = await regular(path.resolve(handbook), name);
          bytes += (await lstat(source)).size;
          if (++files > 512 || bytes > 32 * 1024 * 1024) throw new Error('Documentary content exceeds finite staging bounds');
          const target = path.join(staging, name);
          await mkdir(path.dirname(target), { recursive: true });
          await copyFile(source, target);
        }
      }
    }
    await copyDocumentary(path.resolve(handbook));
  }
  const roles = new Set();
  const paths = new Set();
  for (const resource of template.resources) {
    if (!safe(resource.path)) throw new Error(`Unsafe application path: ${resource.path}`);
    if (roles.has(resource.role) || paths.has(resource.path) || resource.role === 'runtime'
      || resource.role === 'browser-sdk' || resource.role.startsWith('sdk-') || resource.path.startsWith('sdk/')
      || resource.path.startsWith('host/')) throw new Error('Duplicate or reserved application resource');
    roles.add(resource.role); paths.add(resource.path);
    const sourcePath = path.resolve(sourceRoot, resource.source ?? resource.path);
    const source = await regular(path.parse(sourcePath).root, path.relative(path.parse(sourcePath).root, sourcePath));
    delete resource.source;
    const file = path.join(staging, resource.path);
    await mkdir(path.dirname(file), { recursive: true });
    await copyFile(source, file);
  }
  if (!paths.has('index.html')) throw new Error('Application template must include its index.html shell');
  // The Handbook shell must have the same navigation before its runtime starts.
  // Use the already copied and package-hashed site resource as the sole source.
  if (template.application_id === 'conduit.application/handbook') {
    const marker = '<!-- conduit-site-navigation -->';
    const shellPath = path.join(staging, 'index.html');
    const shell = await readFile(shellPath, 'utf8');
    const resource = template.resources.find(resource => resource.role === 'site-navigation');
    if (!resource || shell.split(marker).length !== 2) throw new Error('Handbook shell needs one shared navigation marker and resource');
    const navigation = (await readFile(path.join(staging, resource.path), 'utf8'))
      .replace('data-section="handbook"', 'data-section="handbook" aria-current="page"');
    if (!navigation.includes('aria-current="page"')) throw new Error('Handbook navigation has no active section');
    await writeFile(shellPath, shell.replace(marker, navigation));
  }
  // Only application/documentary documents exist here. Add policy before hashing
  // their manifests and before copying immutable reviewed SDK/release payloads.
  async function secureDocuments(directory) {
    for (const entry of await readdir(directory, { withFileTypes: true })) {
      const file = path.join(directory, entry.name);
      if (entry.isDirectory()) await secureDocuments(file);
      else if (/\.html?$/i.test(entry.name)) {
        const html = await readFile(file, 'utf8');
        const resource = template.resources.find(resource => resource.path === path.relative(staging, file));
        // Admitted content can be an HTML fragment, such as shared navigation.
        // Its containing document owns policy; a fragment has no document head.
        if (resource?.kind === 'content' && resource.path !== 'index.html'
          && !/<(?:!doctype|html|head|body)\b/i.test(html)) continue;
        const secured = applyStaticApplicationCsp(html, {
          loopbackOwnerWindow: template.loopback_owner_window === true && resource?.path === 'index.html',
        });
        if (Buffer.byteLength(secured) > 16 * 1024 * 1024) throw new Error('Static HTML exceeds its finite file bound');
        await writeFile(file, secured);
      }
    }
  }
  await secureDocuments(staging);
  const bundle = await prepareStaticBrowserBundle(path.resolve(release), path.join(staging, '.reviewed-bundle'), template.host_implementations);
  run('targets/browser/sdk/package-browser-bundle.mjs', bundle, path.join(staging, 'sdk'));
  if (bundle === path.join(staging, '.reviewed-bundle')) await rm(bundle, { recursive: true });
  const files = [];
  async function inventory(directory, prefix = '') {
    for (const entry of (await readdir(directory, { withFileTypes: true })).sort((a,b) => a.name.localeCompare(b.name))) {
      const name = prefix + entry.name;
      if (entry.isDirectory()) await inventory(path.join(directory, entry.name), `${name}/`);
      else if (entry.isFile()) files.push(name);
      else throw new Error('SDK package contains a nonregular file');
    }
  }
  await inventory(path.join(staging, 'sdk'));
  const sdkResources = [];
  for (const [index, name] of files.entries()) {
    const role = name === 'browser-sdk.mjs' ? 'browser-sdk' : name === 'bundle/runtime.wasm' ? 'runtime' : `sdk-${index}`;
    sdkResources.push({ role, kind: role === 'runtime' ? 'wasm' : 'content', path: `sdk/${name}`,
      maximum_bytes: (await lstat(path.join(staging, 'sdk', name))).size, dependencies: [] });
  }
  const byPath = new Map(sdkResources.map(resource => [resource.path, resource]));
  for (const resource of sdkResources.filter(resource => /\.(mjs|js)$/.test(resource.path))) {
    const text = await readFile(path.join(staging, resource.path), 'utf8');
    const specifiers = [...text.matchAll(/(?:\bfrom\s*|\bimport\s*)["']([^"']+)["']/g)].map(match => match[1]);
    for (const specifier of [...new Set(specifiers)]) {
      if (!specifier.startsWith('.')) throw new Error(`SDK has nonlocal dependency: ${specifier}`);
      const dependency = byPath.get(path.posix.normalize(path.posix.join(path.posix.dirname(resource.path), specifier)));
      if (!dependency) throw new Error(`SDK dependency not packaged: ${resource.path}: ${specifier}`);
      resource.dependencies.push({ role: dependency.role, specifier });
    }
  }
  template.resources.push(...sdkResources);
  if (template.resources.length > 96 || template.resources.reduce((sum, resource) => sum + resource.maximum_bytes, 0) > 32 * 1024 * 1024) {
    throw new Error('Static application exceeds existing application package bounds');
  }
  for (const resource of template.resources) {
    if (resource.dependencies?.length > 16 || !Number.isSafeInteger(resource.maximum_bytes) || resource.maximum_bytes < 1
      || resource.maximum_bytes > 16 * 1024 * 1024) throw new Error('Application resource exceeds existing bounds');
  }
  for (const name of ['browser-application-loader.mjs', 'browser-application-sdk.mjs', 'browser-application-storage.mjs', 'application-presentation.mjs', 'application-graph-canvas.mjs', 'application-theme.mjs']) {
    await mkdir(path.join(staging, 'host/assets'), { recursive: true });
    await copyFile(path.join(repository, 'targets/browser/host/assets', name), path.join(staging, 'host/assets', name));
  }
  const generated = path.join(staging, '.application-template.json');
  await writeFile(generated, JSON.stringify(template));
  run('targets/browser/tools/build-browser-application-package.mjs', generated, staging, 'application.application.json');
  await rm(generated);
  await rename(staging, output);
} catch (error) {
  await rm(staging, { recursive: true, force: true });
  throw error;
}
