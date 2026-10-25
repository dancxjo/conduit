import { promises as fs } from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { createReadStream } from 'node:fs';
import { runHostedCargoCache } from '../../compiler-cache/cargo-sandbox.mjs';
import { proveHostedHello } from '../../compiler-cache/hosted-proof.mjs';
import { verifyHostedSource } from '../../compiler-cache/hosted-snapshot.mjs';
import { assertSourceCheckout } from '../source.mjs';
import { recordProductCommand } from '../producer-custody.mjs';

const fail = reason => { throw new Error(`Hosted cache product: ${reason}`); };
async function digest(file) {
  const hash = createHash('sha256');
  for await (const bytes of createReadStream(file)) hash.update(bytes);
  return hash.digest('hex');
}

// The compiler cache is an optimization of construction. This adapter still
// proves the newly retained binary, binds exact source, and stages ordinary
// product files for the caller's existing custody/sealing path. It never
// restores a Cargo target or adopts an earlier task's proof.
export async function cachedHostedLinux({ snapshot, cacheRoot, taskRoot, directory,
  repositoryRoot = process.cwd(), maxCacheBytes }, {
  build = runHostedCargoCache, prove = proveHostedHello,
  verifySource = verifyHostedSource, assertCheckout = assertSourceCheckout,
  observeCommand = recordProductCommand,
} = {}) {
  if (process.platform !== 'linux' || process.arch !== 'x64' ||
      snapshot?.schema !== 'hosted-linux-cargo-snapshot-v1' ||
      ![snapshot.root, cacheRoot, taskRoot, directory, repositoryRoot].every(value =>
        typeof value === 'string' && path.isAbsolute(value))) fail('unsupported selection');
  if ((await fs.readdir(directory)).length) fail('product output must be empty');
  const checkSource = async () => {
    assertCheckout(snapshot.sourceCommit, repositoryRoot);
    await verifySource({ repositoryRoot, sourceRoot: path.join(snapshot.root, 'source'),
      sourceCommit: snapshot.sourceCommit, sourceTree: snapshot.sourceTree });
  };
  await checkSource();
  let observed = 0;
  const observe = observation => {
    if (!observation?.program || !Array.isArray(observation.args) ||
        typeof observation.cwd !== 'string' || !observation.environment ||
        !Number.isFinite(observation.durationMs) || observation.durationMs < 0 ||
        !['success', 'failed'].includes(observation.outcome)) fail('invalid sandbox command observation');
    observeCommand(observation);
    observed++;
  };
  const compilation = await build({ snapshot, cacheRoot, taskRoot, observeCommand: observe,
    ...(maxCacheBytes === undefined ? {} : { maxCacheBytes }) });
  if (!observed) fail('build omitted actual command custody');
  const binary = path.join(taskRoot, 'target/release/conduit');
  const before = await digest(binary);
  const product = compilation.outputs?.filter(output => output.name === 'conduit');
  if (compilation.snapshotSha256 !== snapshot.sha256 || product?.length !== 1 ||
      product[0].sha256 !== before) fail('compilation binary custody differs');
  // Always execute the canonical runtime proof, including when every eligible
  // compiler invocation hit. The proof helper verifies the immutable snapshot
  // and retained binary before and after its separate sandbox execution.
  const buildObservations = observed;
  const proof = await prove({ snapshot, taskRoot, observeCommand: observe });
  if (observed === buildObservations) fail('proof omitted actual command custody');
  if (proof.snapshotSha256 !== snapshot.sha256 || proof.namespace !== compilation.namespace ||
      proof.binarySha256 !== before || proof.binaryUnchanged !== true ||
      await digest(binary) !== before) fail('fresh runtime proof custody differs');
  if (await digest(path.join(taskRoot, 'hello-report.json')) !== proof.reportSha256) {
    fail('runtime report custody differs');
  }
  await checkSource();
  let files = 0, bytes = 0;
  async function retain(input, output) {
    const stat = await fs.lstat(input);
    if (stat.isSymbolicLink()) fail('symlink in product evidence');
    if (stat.isDirectory()) {
      await fs.mkdir(output);
      for (const name of (await fs.readdir(input)).sort()) {
        await retain(path.join(input, name), path.join(output, name));
      }
    } else if (stat.isFile()) {
      bytes += stat.size;
      if (++files > 10000 || bytes > 512 * 1024 ** 2) fail('product evidence exceeds bound');
      const expected = await digest(input);
      await fs.copyFile(input, output, fs.constants.COPYFILE_EXCL);
      await fs.chmod(output, stat.mode & 0o777);
      if (await digest(output) !== expected || await digest(input) !== expected) fail('product changed during retention');
    } else fail('unsupported product evidence');
  }
  await retain(binary, path.join(directory, 'conduit'));
  for (const name of ['hello-report.json', 'hello-stdout.txt', 'hello-artifacts',
    'hello-proof.json', 'cargo-cache-receipt.json']) {
    await retain(path.join(taskRoot, name), path.join(directory, name));
  }
  await fs.writeFile(path.join(directory, 'compiler-snapshot.json'), JSON.stringify({
    schema: 'conduit.hosted-compiler-product/v1', source: snapshot.sourceCommit,
    tree: snapshot.sourceTree, snapshotSha256: snapshot.sha256,
    namespace: compilation.namespace, binarySha256: before,
    compilationReceiptSha256: await digest(path.join(directory, 'cargo-cache-receipt.json')),
    runtimeReceiptSha256: await digest(path.join(directory, 'hello-proof.json')),
    sharedObjectsAreProof: false,
  }, null, 2) + '\n', { flag: 'wx', mode: 0o600 });
  if (await digest(path.join(directory, 'conduit')) !== before) fail('retained binary differs');
  await checkSource();
  return { compilation, proof };
}
