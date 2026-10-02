/** Signed-repository acquisition with a content-verified, baseline-specific deb cache. */
import { recordOperation, recordTool } from './metrics.mjs';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync, lstatSync } from 'node:fs';
import { homedir } from 'node:os';
import path from 'node:path';

const hash = value => createHash('sha256').update(value).digest('hex');
export const ubuntuMirror = text => text.replace(/(https?:\/\/)azure\.archive\.ubuntu\.com(?=[/:\s]|$)/g, '$1archive.ubuntu.com');
function execute(program, args, { input } = {}) {
  const result = spawnSync(program, args, { encoding: 'utf8', input, timeout: 20 * 60_000, maxBuffer: 32 * 1024 * 1024 });
  if (result.error || result.status !== 0) throw new Error(`${program} failed: ${result.error?.message ?? result.stderr}`);
  return result.stdout;
}
function sources(root) {
  const files = [path.join(root, 'etc/apt/sources.list')];
  const directory = path.join(root, 'etc/apt/sources.list.d');
  if (existsSync(directory)) files.push(...readdirSync(directory).filter(name => /\.(list|sources)$/.test(name)).sort().map(name => path.join(directory, name)));
  return files.filter(existsSync).map(file => ({ file, original: readFileSync(file, 'utf8'), text: ubuntuMirror(readFileSync(file, 'utf8')) }));
}
function installed(run) {
  const output = run('dpkg-query', ['-W', '-f=${Package}\t${Version}\t${Architecture}\t${db:Status-Status}\n']);
  return new Map(output.trim().split('\n').filter(Boolean).map(line => line.split('\t'))
    .filter(row => row[3] === 'installed').map(([name, version, architecture]) => [`${name}:${architecture}`, version]));
}
function packageVersion(packages, name, nativeArchitecture) {
  const [base, requestedArchitecture = nativeArchitecture] = name.split(':');
  return packages.get(`${base}:${requestedArchitecture}`) ?? packages.get(`${base}:all`);
}
function assertFile(file) {
  if (!lstatSync(file).isFile()) throw new Error(`APT cache entry is not a regular file: ${file}`);
}
export function acquireApt(requested, options = {}) {
  const started = Date.now();
  const run = options.run ?? execute;
  const root = options.root ?? '/';
  const privileged = options.privileged ?? ((program, args, config) => process.getuid?.() === 0 ? run(program, args, config) : run('sudo', [program, ...args], config));
  const packages = [...new Set(requested)].sort();
  if (!packages.length || packages.some(name => !/^[a-z0-9][a-z0-9+.-]*(?::[a-z0-9-]+)?$/.test(name))) throw new Error('Invalid APT package list');
  const arch = run('dpkg', ['--print-architecture']).trim();
  if (!/^[a-z0-9-]+$/.test(arch)) throw new Error('Invalid native APT architecture');
  const before = installed(run);
  const report = { schema: 'conduit.ci/apt-acquisition@1', requested: packages, cache: 'installed', downloadedPackages: 0, downloadedBytes: 0, installedVersions: {}, durationMs: 0 };
  const finish = () => {
    const after = installed(run);
    for (const name of packages) {
      const version = packageVersion(after, name, arch);
      if (!version) throw new Error(`APT package remains unavailable: ${name}`);
      report.installedVersions[name] = version;
    }
    report.durationMs = Date.now() - started;
    recordOperation({ kind: 'apt', program: 'apt-get', args: packages, durationMs: report.durationMs, outcome: 'success', cacheHit: report.cache !== 'miss', downloadedBytes: report.downloadedBytes });
    (options.recordTool ?? recordTool)('apt-packages', { versions: report.installedVersions, resolutionKey: report.resolutionKey ?? null, resolvedPackages: report.resolvedPackages ?? [] });
    (options.report ?? (value => console.log(JSON.stringify(value))))(report);
    return report;
  };
  const missing = packages.filter(name => !packageVersion(before, name, arch));
  if (!missing.length) return finish();
  const source = sources(root);
  const baseline = {
    architecture: arch,
    os: readFileSync(path.join(root, 'etc/os-release'), 'utf8'),
    image: options.imageVersion ?? process.env.ImageVersion ?? '',
    installed: [...before].sort(([a], [b]) => a.localeCompare(b)),
    sources: source.map(item => [path.relative(root, item.file), item.text]),
    requested: packages,
  };
  const key = hash(JSON.stringify(baseline));
  report.resolutionKey = key;
  const directory = path.join(options.cacheRoot ?? path.join(homedir(), '.cache/conduit-ci/apt'), key);
  const archives = path.join(directory, 'archives');
  const manifestPath = path.join(directory, 'manifest.json');
  mkdirSync(archives, { recursive: true });
  for (const folder of [directory, archives]) if (!lstatSync(folder).isDirectory() || lstatSync(folder).isSymbolicLink()) throw new Error(`APT cache directory is unsafe: ${folder}`);
  for (const name of ['partial', 'lock']) {
    const entry = path.join(archives, name);
    if (existsSync(entry) && lstatSync(entry).isSymbolicLink()) throw new Error(`APT cache working entry is unsafe: ${entry}`);
  }
  // Only Ubuntu's official Azure mirror is replaced; all suites, components,
  // signing configuration and third-party repositories remain as authored.
  for (const item of source) if (item.original !== item.text) privileged('tee', [item.file], { input: item.text });
  const aptOptions = ['-o', 'APT::Keep-Downloaded-Packages=true', '-o', 'Binary::apt-get::APT::Keep-Downloaded-Packages=true', '-o', 'APT::Update::Error-Mode=any', '-o', 'Acquire::Retries=1', '-o', 'Acquire::http::Timeout=30', '-o', 'Acquire::https::Timeout=30', '-o', 'APT::Get::AllowUnauthenticated=false', '-o', 'Acquire::AllowInsecureRepositories=false', '-o', 'Acquire::AllowDowngradeToInsecureRepositories=false', '-o', `Dir::Cache::archives=${archives}`];
  privileged('apt-get', [...aptOptions, 'update']);
  let manifest;
  if (existsSync(manifestPath)) {
    assertFile(manifestPath);
    manifest = JSON.parse(readFileSync(manifestPath, 'utf8'));
    if (manifest.schema !== report.schema || manifest.key !== key || JSON.stringify(manifest.baseline) !== JSON.stringify(baseline) || !Array.isArray(manifest.debs) || !manifest.debs.length) throw new Error('APT cache manifest mismatch');
    report.cache = 'hit';
  } else {
    // An incomplete acquisition is not a valid warm cache. APT validates any
    // partial archive against its authenticated index before reusing it.
    const preexistingDebs = readdirSync(archives).filter(name => name.endsWith('.deb'));
    privileged('apt-get', [...aptOptions, 'install', '--download-only', '-y', '--no-install-recommends', ...missing]);
    const debs = readdirSync(archives).filter(name => name.endsWith('.deb')).sort().map(file => {
      const full = path.join(archives, file);
      assertFile(full);
      const fields = run('dpkg-deb', ['-f', full, 'Package', 'Version', 'Architecture']).trim().split('\n').map(line => line.replace(/^[A-Za-z]+: /, ''));
      const [name, version, architecture] = fields;
      return { file, name, version, architecture, sha256: hash(readFileSync(full)), bytes: lstatSync(full).size };
    });
    if (!debs.length) throw new Error('APT acquisition produced no package closure');
    manifest = { schema: report.schema, key, baseline, debs };
    report.cache = 'miss';
    report.downloadedPackages = preexistingDebs.length ? null : debs.length;
    report.downloadedBytes = preexistingDebs.length ? null : debs.reduce((sum, deb) => sum + deb.bytes, 0);
  }
  const actualFiles = readdirSync(archives).filter(name => name.endsWith('.deb')).sort();
  if (JSON.stringify(actualFiles) !== JSON.stringify(manifest.debs.map(deb => deb.file).sort())) throw new Error('APT cache file set mismatch');
  const exact = [];
  for (const deb of manifest.debs) {
    if (!/^[a-z0-9][a-z0-9+.-]*$/.test(deb.name) || !/^[a-z0-9-]+$/.test(deb.architecture) || !/^[A-Za-z0-9.+:~_-]+$/.test(deb.version) || path.basename(deb.file) !== deb.file || !deb.file.endsWith('.deb')) throw new Error('APT cache package identity malformed');
    const file = path.join(archives, deb.file);
    assertFile(file);
    if (hash(readFileSync(file)) !== deb.sha256 || lstatSync(file).size !== deb.bytes) throw new Error(`APT cache digest mismatch: ${deb.file}`);
    const identity = `${deb.name}${deb.architecture === 'all' ? '' : `:${deb.architecture}`}=${deb.version}`;
    const metadata = run('apt-cache', ['show', identity]);
    if (!metadata.split('\n\n').some(stanza => stanza.split('\n').includes(`Package: ${deb.name}`) && stanza.split('\n').includes(`Version: ${deb.version}`) && stanza.split('\n').includes(`Architecture: ${deb.architecture}`) && stanza.split('\n').includes(`SHA256: ${deb.sha256}`))) throw new Error(`APT cached version is not authenticated by current repository: ${identity}`);
    exact.push(identity);
  }
  privileged('apt-get', [...aptOptions, 'install', '--no-download', '-y', '--no-install-recommends', ...missing, ...exact]);
  const after = installed(run);
  for (const deb of manifest.debs) if (packageVersion(after, `${deb.name}:${deb.architecture}`, arch) !== deb.version) throw new Error(`APT installed version differs: ${deb.name}`);
  // APT owns its lock and _apt/0700 partial directory. Cache actions run as
  // the unprivileged runner and must be able to archive this acquisition tree.
  // These public package files contain no credentials; limit permission changes
  // to this baseline's checked archive directory, after apt has finished.
  privileged('chmod', ['-R', 'a+rX', archives]);
  report.resolvedPackages = manifest.debs.map(({ name, version, architecture, sha256, bytes }) => ({ name, version, architecture, sha256, bytes }));
  writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
  return finish();
}
