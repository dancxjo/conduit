// Private host state. A crash retains its lock/intent for explicit inspection.
import { constants } from 'node:fs';
import { open, lstat, realpath, unlink, rename } from 'node:fs/promises';
import path from 'node:path';
import { randomUUID } from 'node:crypto';
const fail = () => { throw new Error('Train supervisor private state refused'); };
const owned = info => info.uid === process.getuid() && !(info.mode & 0o077);
export async function readSupervisorJson(filename, maximum = 131072) {
  let fd;
  try {
    fd = await open(filename, constants.O_RDONLY | constants.O_NOFOLLOW | constants.O_NONBLOCK);
    const before = await fd.stat();
    if (!before.isFile() || before.nlink !== 1 || !owned(before) || before.size < 2 || before.size > maximum) fail();
    const buffer = Buffer.alloc(before.size + 1);
    const { bytesRead } = await fd.read(buffer, 0, buffer.length, 0);
    const after = await fd.stat();
    if (bytesRead !== before.size || after.size !== before.size || after.mtimeMs !== before.mtimeMs || after.ctimeMs !== before.ctimeMs) fail();
    return JSON.parse(buffer.subarray(0, bytesRead).toString('utf8'));
  } catch (error) { if (error.code === 'ENOENT') throw error; fail(); }
  finally { await fd?.close(); }
}
export async function withSupervisorStore(directory, callback) {
  if (process.platform !== 'linux' || !path.isAbsolute(directory) || await realpath(directory) !== directory) fail();
  const info = await lstat(directory);
  if (!info.isDirectory() || !owned(info)) fail();
  const lockPath = path.join(directory, 'controller.lock');
  let lock;
  try { lock = await open(lockPath, 'wx', 0o600); } catch { throw new Error('Train supervisor lock exists or cannot be acquired; inspect before recovery'); }
  const lockInfo = await lock.stat();
  const syncDirectory = async () => { const fd = await open(directory, 'r'); try { await fd.sync(); } finally { await fd.close(); } };
  try {
    await lock.writeFile(JSON.stringify({ pid: process.pid, nonce: randomUUID() }) + '\n'); await lock.sync(); await syncDirectory();
    const filename = path.join(directory, 'journal.json');
    const store = {
      async read() { try { return await readSupervisorJson(filename); } catch (error) { if (error.code === 'ENOENT') return null; throw error; } },
      async write(value) {
        const bytes = Buffer.from(JSON.stringify(value) + '\n'); if (bytes.length > 131072) fail();
        try { const current = await lstat(filename); if (!current.isFile() || current.nlink !== 1 || !owned(current)) fail(); }
        catch (error) { if (error.code !== 'ENOENT') throw error; }
        const temporary = path.join(directory, `.journal-${randomUUID()}`);
        const fd = await open(temporary, 'wx', 0o600);
        try { await fd.writeFile(bytes); await fd.sync(); } finally { await fd.close(); }
        await rename(temporary, filename); await syncDirectory();
      },
    };
    return await callback(store);
  } finally {
    await lock.close();
    const current = await lstat(lockPath);
    if (current.ino !== lockInfo.ino || current.dev !== lockInfo.dev || !current.isFile()) fail();
    await unlink(lockPath); await syncDirectory();
  }
}
