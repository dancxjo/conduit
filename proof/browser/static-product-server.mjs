import { spawn } from "node:child_process";

function awaitUrl(child, pattern, label) {
  let output = "";
  return new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error(`${label} was not ready\n${output}`)), 10_000);
    const inspect = (chunk) => {
      output += chunk.toString();
      const match = output.match(pattern);
      if (match) {
        clearTimeout(timeout);
        resolve(match[1]);
      }
    };
    child.stdout.on("data", inspect);
    child.stderr.on("data", inspect);
    child.once("exit", (code) => {
      clearTimeout(timeout);
      reject(new Error(`${label} exited (${code})\n${output}`));
    });
  });
}

export async function startStaticProduct(root, mount = "/") {
  const child = spawn("node", ["proof/browser/static-server.mjs", "0", root, mount], {
    cwd: new URL("../..", import.meta.url).pathname,
    stdio: ["ignore", "pipe", "pipe"],
  });
  const url = await awaitUrl(child, /CONDUIT_STATIC_SERVER_URL=(http:\/\/127\.0\.0\.1:\d+\/\S*)/, "staged product");
  return { child, url };
}
