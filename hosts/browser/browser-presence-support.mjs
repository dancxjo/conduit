import { spawn } from "node:child_process";

export async function startPresenceProbe(extraArguments = []) {
  const process = spawn("target/debug/browser-admission-probe", ["--presence", ...extraArguments], {
    cwd: new URL("../..", import.meta.url).pathname,
    stdio: ["ignore", "pipe", "pipe"],
  });
  let output = "";
  const url = await new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error(`presence probe was not ready\n${output}`)), 10_000);
    const inspect = (chunk) => {
      output += chunk.toString();
      const match = output.match(/^(ws:\/\/[^\s]+)/);
      if (match) {
        clearTimeout(timeout);
        resolve(match[1]);
      }
    };
    process.stdout.on("data", inspect);
    process.stderr.on("data", inspect);
    process.once("exit", (code) => {
      clearTimeout(timeout);
      if (code !== 0) reject(new Error(`presence probe exited (${code})\n${output}`));
    });
  });
  return { process, url, output: () => output };
}
export async function startWebRtcRendezvousProbe() {
  const process = spawn("target/debug/browser-webrtc-rendezvous-probe", [], {
    cwd: new URL("../..", import.meta.url).pathname,
    stdio: ["ignore", "pipe", "pipe"],
  });
  let output = "";
  const url = await new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error(`rendezvous probe was not ready\n${output}`)), 10_000);
    const inspect = (chunk) => {
      output += chunk.toString();
      const match = output.match(/^(ws:\/\/[^\s]+)/);
      if (match) {
        clearTimeout(timeout);
        resolve(match[1]);
      }
    };
    process.stdout.on("data", inspect);
    process.stderr.on("data", inspect);
    process.once("exit", (code) => {
      clearTimeout(timeout);
      if (code !== 0) reject(new Error(`rendezvous probe exited (${code})\n${output}`));
    });
  });
  return { process, url, output: () => output };
}
