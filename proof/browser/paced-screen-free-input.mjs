// Drive the real screen-free CLI as a person would: wait for each completed
// reading and its next prompt before submitting another command. Preloading
// stdin is a valid interruption test, but it cannot prove a full spoken pass.
import { spawn } from 'node:child_process';

const MAX_TRANSCRIPT_BYTES = 8 * 1024 * 1024;

export async function runPacedScreenFree(executable, args, commands, prompts, timeoutMs,
  { retryStaleReadAll = 0 } = {}) {
  prompts = Array.isArray(prompts) ? prompts : [prompts];
  if (prompts.length === 0 || prompts.some(prompt => !['birth> ', 'body> '].includes(prompt)) ||
      new Set(prompts).size !== prompts.length || commands.length === 0 ||
      commands.some(command => !command || command.includes('\n') || command.includes('\r')) ||
      !Number.isInteger(retryStaleReadAll) || retryStaleReadAll < 0 || retryStaleReadAll > 4 ||
      (retryStaleReadAll > 0 &&
        (commands.at(-2) !== 'read all' || commands.at(-1) !== 'quit'))) {
    throw new Error('invalid paced screen-free commands or prompt');
  }
  const child = spawn(executable, args, { stdio: ['pipe', 'pipe', 'pipe'] });
  let stdout = '';
  let stderr = '';
  let sent = 0;
  let scanned = 0;
  let previousPromptEnd = 0;
  let previousCommand;
  let staleRetries = 0;
  const sentCommands = [];
  let failed;
  const stop = reason => {
    failed ??= reason;
    child.kill();
  };
  const deadline = setTimeout(() => stop(`screen-free ${prompts.join('/').trim()} deadline`), timeoutMs);
  child.stdout.on('data', chunk => {
    const prior = stdout.length;
    stdout += chunk.toString('utf8');
    if (Buffer.byteLength(stdout) > MAX_TRANSCRIPT_BYTES) {
      stop('screen-free transcript exceeded the finite bound');
      return;
    }
    // A prompt may be split across two pipe chunks. Scan the overlap only
    // once, even when several prompts arrive in a single chunk.
    const overlap = Math.max(...prompts.map(prompt => prompt.length)) - 1;
    const nextPrompt = () => prompts.map(prompt => ({
      prompt, at: stdout.indexOf(prompt, Math.max(scanned, prior - overlap)),
    })).filter(found => found.at !== -1).sort((a, b) => a.at - b.at)[0];
    let found = nextPrompt();
    while (found) {
      const end = found.at + found.prompt.length;
      scanned = end;
      const response = stdout.slice(previousPromptEnd, found.at);
      previousPromptEnd = end;
      const staleRead = retryStaleReadAll > 0 && previousCommand === 'read all' &&
        commands[sent] === 'quit' &&
        response.includes('Stopped the stale reading; read all again for the current Face.');
      if (staleRead && staleRetries >= retryStaleReadAll) {
        stop('current Face did not remain readable after bounded stale-read retries');
        return;
      }
      if (staleRead || sent < commands.length) {
        const command = staleRead ? 'read all' : commands[sent++];
        if (staleRead) staleRetries++;
        child.stdin.write(`${command}\n`);
        sentCommands.push(command);
        previousCommand = command;
        if (command === 'quit') child.stdin.end();
      }
      found = nextPrompt();
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
    return { transcript: stdout, commands: sentCommands };
  } finally {
    clearTimeout(deadline);
  }
}
