import { cpSync, existsSync, readdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { cargo, command, digest, xtask } from './common.mjs';

// Exact staged Workspace acceptance plus independent browser adapter contracts.
// Cross-target deployment tests require another lane's artifacts and are not
// represented as successful by this lane.
const SPECS = [
  'workspace-arrival', 'workspace-birth-naming', 'sdk-external-body-execution',
  'workspace-membership', 'workspace-library', 'workspace-resident-applications',
  'workspace-continuity',
  'creche-browser-configuration', 'creche-rendezvous',
  'signal-dom-host', 'browser-host-calls', 'browser-pointer', 'browser-human-input',
  'browser-host-entrance', 'browser-media-host', 'browser-device-base', 'browser-usb-device-base',
  'rp2040-browser-deployment', 'esp32-browser-deployment',
];

function inventory(directory, prefix = '') {
  return readdirSync(directory, { withFileTypes: true }).flatMap(entry => {
    const name = prefix + entry.name;
    const file = path.join(directory, entry.name);
    if (entry.isSymbolicLink()) throw new Error(`Unexpected product symlink: ${file}`);
    return entry.isDirectory() ? inventory(file, `${name}/`) : [[name, digest(file)]];
  }).sort(([a], [b]) => a.localeCompare(b));
}

export function browser(directory) {
  const product = 'target/workspace-product';
  const releases = path.join(directory, 'host-release');
  if (existsSync(product)) throw new Error(`Stale staged product exists: ${product}; use a clean target lane`);
  cargo('build', '--locked', '-p', 'conduit', '--bin', 'conduit',
    '-p', 'conduit-browser-host', '--bin', 'conduit-browser-host',
    '-p', 'conduit-browser-patchbay-workbench', '--bin', 'conduit-browser-patchbay-workbench',
    '-p', 'conduit-std-host', '--bin', 'webchat-server', '--bin', 'browser-admission-probe');
  xtask('make', 'host', 'release', '--platform', 'browser', '--output', releases);
  xtask('make', 'host', 'release-catalog', '--root', releases, '--generation', '1');
  // Stage the runtime already sealed by the release command; no second WASM build.
  command('sh', ['targets/browser/tools/stage-browser-workspace.sh',
    path.join(releases, 'runtime.wasm'), product, releases]);
  const before = inventory(product);
  command('node', ['--test', ...[
    'browser-body-input', 'browser-body-host', 'workspace-handoff', 'browser-plot-effects',
    'browser-pitch-tone', 'browser-host-calls', 'creche-rendezvous', 'physical-host-workflow',
    'creche-release-catalog',
  ].map(name => `proof/browser/${name}.test.mjs`)]);
  command('node', ['proof/browser/node_modules/@playwright/test/cli.js', 'test',
    '--config', 'proof/browser/playwright.config.mjs', '--project', 'chromium',
    '--workers', '1', '--retries', '0', ...SPECS.map(name => `${name}.spec.mjs`)], {
    env: { ...process.env, RUSTUP_TOOLCHAIN: process.env.RUSTUP_TOOLCHAIN || 'stable',
      CONDUIT_CRECHE_RENDEZVOUS_PRODUCT: product },
  });
  command('node', ['tools/ci/pipeline/targets/browser-smoke.mjs', product, path.join(directory, 'accessibility.json')]);
  if (JSON.stringify(before) !== JSON.stringify(inventory(product))) throw new Error('Browser proof modified its staged product');
  cpSync(product, path.join(directory, 'workspace'), { recursive: true, errorOnExist: true, force: false });
  if (JSON.stringify(before) !== JSON.stringify(inventory(path.join(directory, 'workspace')))) throw new Error('Browser product copy changed bytes');
  writeFileSync(path.join(directory, 'proof-scope.json'), JSON.stringify({
    product: 'workspace/', project: 'chromium', workers: 1, retries: 0, specs: SPECS,
    additionalProof: 'Workspace WCAG 2.2 AA and local asset/link smoke',
    excluded: 'Cross-target download/deployment, Pages homepage, Patchbay, physical/HIL, other browser projects; mixed-membership requires the absent browser-parts-capstone binary',
  }, null, 2));
}
