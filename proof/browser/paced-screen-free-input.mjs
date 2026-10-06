// Drive the real screen-free CLI as a person would: wait for each completed
// reading and its next prompt before submitting another command. Preloading
// stdin is a valid interruption test, but it cannot prove a full spoken pass.
import { spawn } from 'node:child_process';

const MAX_TRANSCRIPT_BYTES = 8 * 1024 * 1024;

export async function runPacedScreenFree(executable, args, commands, prompt, timeoutMs) {
  if (!['birth> ', 'body> '].includes(prompt) || commands.length === 0 ||
      commands.some(command => !command || command.includes('\n') || command.includes('\r'))) {
    throw new Error('invalid paced screen-free commands or prompt');
  }
  const child = spawn(executable, args, { stdio: ['pipe', 'pipe', 'pipe'] });
  let stdout = '';
  let stderr = '';
  let sent = 0;
  let scanned = 0;
  let failed;
  const stop = reason => {
    failed ??= reason;
    child.kill();
  };
  const deadline = setTimeout(() => stop(`screen-free ${prompt.trim()} deadline`), timeoutMs);
  child.stdout.on('data', chunk => {
    const prior = stdout.length;
    stdout += chunk.toString('utf8');
    if (Buffer.byteLength(stdout) > MAX_TRANSCRIPT_BYTES) {
      stop('screen-free transcript exceeded the finite bound');
      return;
    }
    // A prompt may be split across two pipe chunks. Scan the overlap only
    // once, even when several prompts arrive in a single chunk.
    let at = stdout.indexOf(prompt, Math.max(scanned, prior - prompt.length + 1));
    while (at !== -1) {
      const end = at + prompt.length;
      scanned = end;
      if (sent < commands.length) {
        child.stdin.write(`${commands[sent++]}\n`);
        if (sent === commands.length) child.stdin.end();
      }
      at = stdout.indexOf(prompt, scanned);
    }
  });
  child.stderr.on('data', chunk => {
    stderr += chunk.toString('utf8');
    if (Buffer.byteLength(stderr) > MAX_TRANSCRIPT_BYTES) stop('screen-free stderr exceeded the finite bound');
  });
  try {
    const code = await new Promise((resolve, reject) => {
      child.once('error', reject);
      child.once('close', resolve);
    });
    if (failed || code !== 0 || sent !== commands.length) {
      throw new Error(`${failed ?? `screen-free exited ${code} after ${sent}/${commands.length} commands`}: ${stderr || stdout.slice(-2000)}`);
    }
    return stdout;
  } finally {
    clearTimeout(deadline);
  }
}
