import { spawnSync } from 'node:child_process';
import { mkdtemp, mkdir, realpath, writeFile, rm } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';

const fail = () => { throw new Error('Isolated train push context refused'); };
const safePath = value => typeof value === 'string' && path.isAbsolute(value)
  && !/[\r\n\0]/.test(value);

// Borrow objects only. Source refs, hooks, remotes and configuration never enter
// the repository in which the installation credential is subsequently used.
export async function withIsolatedTrainPushContext({ directory, source, environment,
  command = spawnSync }, callback) {
  if (!safePath(directory) || !/^[a-f0-9]{40}$/.test(source ?? '')
      || !environment || typeof environment !== 'object' || typeof callback !== 'function') fail();
  const env = Object.fromEntries(Object.entries(environment).filter(([key]) =>
    !key.startsWith('GIT_') && !['GH_TOKEN', 'GITHUB_TOKEN'].includes(key)));
  Object.assign(env, { GIT_CONFIG_NOSYSTEM: '1', GIT_CONFIG_GLOBAL: '/dev/null',
    GIT_CONFIG_SYSTEM: '/dev/null', GIT_TERMINAL_PROMPT: '0', GIT_NO_REPLACE_OBJECTS: '1',
    GIT_NO_LAZY_FETCH: '1' });
  const git = (args, cwd) => {
    let result;
    try { result = command('git', args, { cwd, env: { ...env }, encoding: 'utf8',
      timeout: 60000, maxBuffer: 65536 }); } catch { fail(); }
    if (result?.error || result?.signal || result?.status !== 0 || typeof result.stdout !== 'string') fail();
    return result.stdout;
  };
  const raw = git(['rev-parse', '--path-format=absolute', '--git-path', 'objects'], directory);
  // Git appends one LF. Embedded line breaks cannot be represented in alternates.
  const objectPath = raw.endsWith('\n') ? raw.slice(0, -1) : raw;
  if (!safePath(objectPath)) fail();
  let objects;
  try { objects = await realpath(objectPath); } catch { fail(); }
  if (!safePath(objects)) fail();
  const temporary = await mkdtemp(path.join(os.tmpdir(), 'conduit-train-push-'));
  try {
    if (!safePath(temporary)) fail();
    const bare = path.join(temporary, 'repository.git');
    git(['init', '--bare', '--template=', bare], temporary);
    const hooks = path.join(temporary, 'empty-hooks'); await mkdir(hooks, { mode: 0o700 });
    // JSON quoting is also valid Git config string quoting for these paths.
    await writeFile(path.join(bare, 'config'), `[core]\nrepositoryformatversion = 0\nbare = true\nhooksPath = ${JSON.stringify(hooks)}\n`, { mode: 0o600 });
    await writeFile(path.join(bare, 'objects', 'info', 'alternates'), `${objects}\n`, { flag: 'wx', mode: 0o600 });
    if (git(['cat-file', '-t', source], bare).trim() !== 'commit') fail();
    return await callback({ directory: bare, environment: Object.freeze({ ...env }) });
  } finally {
    await rm(temporary, { recursive: true, force: true });
  }
}
