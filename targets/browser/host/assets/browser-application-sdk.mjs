// Trusted SDK composition over bytes already admitted by the application loader.
// User application modules retain their stricter no-dynamic-import boundary.
export async function admitApplicationSdk(manifest, admittedBytes) {
  const resources = new Map(manifest.resources.map(resource => [resource.role, resource]));
  if (!resources.has('browser-sdk')) return null;
  const decoder = new TextDecoder('utf-8', { fatal: true });
  const urls = new Map();
  const visiting = new Set();
  async function moduleUrl(role) {
    if (urls.has(role)) return urls.get(role);
    const resource = resources.get(role);
    if (!resource || !resource.path.startsWith('sdk/') || !/\.(mjs|js)$/.test(resource.path)
      || resource.kind !== 'content' || visiting.has(role)) throw new Error('Invalid admitted SDK module graph');
    visiting.add(role);
    let source = decoder.decode(admittedBytes.get(role));
    for (const dependency of resource.dependencies) {
      const target = await moduleUrl(dependency.role);
      let found = false;
      for (const marker of [JSON.stringify(dependency.specifier), `'${dependency.specifier}'`]) {
        if (source.includes(marker)) { source = source.split(marker).join(JSON.stringify(target)); found = true; }
      }
      if (!found) throw new Error('Admitted SDK dependency is unused');
    }
    source = source.split('import.meta.url').join(JSON.stringify(resource.url.href));
    if (/(?:\bfrom\s*|\bimport\s*)["'](?!blob:)/.test(source)) throw new Error('SDK dependency was not admitted');
    if (/\bimport\s*\(/.test(source) && resource.path !== 'sdk/browser-sdk.mjs') throw new Error('Unexpected SDK dynamic import');
    visiting.delete(role);
    const url = URL.createObjectURL(new Blob([source], { type: 'text/javascript' }));
    urls.set(role, url);
    return url;
  }
  try {
    const sdk = await import(await moduleUrl('browser-sdk'));
    if (typeof sdk.Conduit?.browser !== 'function') throw new Error('Admitted SDK has no Browser Host entrance');
    const application = { identity: manifest.applicationId, stateCompatibility: manifest.stateCompatibility, packageDigest: manifest.packageDigest };
    const continuityResource = manifest.resources.find(resource => resource.path === 'sdk/browser-sdk-continuity.mjs');
    if (!continuityResource) throw new Error('Admitted SDK has no application continuity boundary');
    const { applicationContinuity } = await import(await moduleUrl(continuityResource.role));
    const continuity = await applicationContinuity(application);
    return { continuity, browser: ({ root } = {}) => sdk.Conduit.browser({ root,
      bundleRoot: new URL('./bundle/', resources.get('browser-sdk').url), application,
    }) };
  } finally { for (const url of urls.values()) URL.revokeObjectURL(url); }
}
