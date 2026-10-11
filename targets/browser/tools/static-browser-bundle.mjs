import { createHash } from 'node:crypto';
import { readFile, writeFile, mkdir, lstat } from 'node:fs/promises';
import path from 'node:path';
import { buildBrowserBundleImage } from '../deployment/browser/browser-bundle.mjs';
import { bindBrowserRuntimeBridge } from '../host/assets/browser-runtime-bridge.mjs';

export async function prepareStaticBrowserBundle(source, destination, implementations) {
  try {
    await lstat(path.join(source, 'browser-bundle-release.json'));
    return source;
  } catch (error) { if (error.code !== 'ENOENT') throw error; }
  const manifest = JSON.parse(await readFile(path.join(source, 'browser-page.json'), 'utf8'));
  if (!Array.isArray(manifest.files) || manifest.files.length > 16) throw new Error('Reviewed distribution inventory is invalid');
  const payloads = [];
  for (const file of manifest.files) {
    if (typeof file.path !== 'string' || !/^[a-zA-Z0-9._/-]+$/.test(file.path)
      || file.path.split('/').some(part => !part || part === '.' || part === '..')) throw new Error('Reviewed distribution path is unsafe');
    const name = path.join(source, file.path);
    const stat = await lstat(name);
    if (!stat.isFile() || stat.isSymbolicLink() || stat.size !== file.bytes || stat.size > (file.path === 'runtime.wasm' ? 20 : 16) * 1024 * 1024) throw new Error('Reviewed distribution file changed size');
    const bytes = new Uint8Array(await readFile(name));
    if (`sha256:${createHash('sha256').update(bytes).digest('hex')}` !== file.sha256) throw new Error('Reviewed distribution file changed digest');
    payloads.push({ ...file, bytes });
  }
  const runtime = payloads.find(file => file.path === 'runtime.wasm');
  if (!runtime) throw new Error('Reviewed distribution has no runtime');
  const { instance } = await WebAssembly.instantiate(runtime.bytes, {});
  const api = instance.exports;
  bindBrowserRuntimeBridge(api, { context: 'static application configuration review' });
  const output = () => {
    const length = api.conduit_creche_output_len();
    if (length < 1 || length > 256 * 1024) throw new Error('Configuration review output exceeds its bound');
    return JSON.parse(new TextDecoder().decode(new Uint8Array(api.memory.buffer, api.conduit_creche_output_ptr(), length)));
  };
  if (api.conduit_creche_browser_configuration_catalog() < 0) throw new Error('Browser configuration catalog refused');
  const catalog = output();
  const selection = new TextEncoder().encode(JSON.stringify({ catalog_generation: catalog.generation, implementations }));
  if (selection.length > api.conduit_creche_input_capacity()) throw new Error('Application implementation selection exceeds its bound');
  new Uint8Array(api.memory.buffer, api.conduit_creche_input_ptr(), selection.length).set(selection);
  if (api.conduit_creche_review_browser_configuration(selection.length) < 0) throw new Error(`Application profile refused: ${JSON.stringify(output())}`);
  const checked = output();
  const release = await buildBrowserBundleImage({ checked, distribution: { manifest, payloads } });
  await mkdir(destination);
  for (const payload of release.payloads) {
    const file = path.join(destination, payload.path);
    await mkdir(path.dirname(file), { recursive: true });
    await writeFile(file, payload.bytes);
  }
  await writeFile(path.join(destination, 'browser-page.json'), JSON.stringify(manifest));
  await writeFile(path.join(destination, 'browser-bundle-release.json'), JSON.stringify(release.manifest));
  return destination;
}
