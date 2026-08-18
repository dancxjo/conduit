import { expect, test } from "@playwright/test";
import { spawn } from "node:child_process";

let server;
let lineUrl;
let output = "";

test.beforeAll(async () => {
  server = spawn("target/debug/r1-three-peer-input-server", ["127.0.0.1:0"], {
    cwd: new URL("../..", import.meta.url).pathname,
    stdio: ["pipe", "pipe", "pipe"],
  });
  lineUrl = await new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error("R1 input server did not become ready")), 10_000);
    const inspect = (chunk) => {
      output += chunk.toString();
      const match = output.match(/r1-three-peer-input-ready address=([^\s]+)/);
      if (match) {
        clearTimeout(timeout);
        resolve(`ws://${match[1]}`);
      }
    };
    server.stdout.on("data", inspect);
    server.stderr.on("data", inspect);
    server.once("exit", (code) => reject(new Error(`R1 input server exited before ready (${code})\n${output}`)));
  });
});

test.afterAll(() => {
  if (server?.exitCode === null) server.kill("SIGTERM");
});

test("terminal and two Chromium peers independently issue keydown on and keyup off", async ({ browser, request }) => {
  const authored = await (await request.get("/fixtures/forms/r1-three-peer-control.conduit")).text();
  expect(authored).toContain("terminal: interaction/level");
  expect(authored).toContain("browser-a: interaction/level");
  expect(authored).toContain("browser-b: interaction/level");
  expect(authored).toContain("merge: flow/merge-three-signal");
  expect(authored).not.toMatch(/websocket|usb|address|dom|tty/i);

  const context = await browser.newContext();
  const pageA = await context.newPage();
  const pageB = await context.newPage();
  const pageUrl = (peer) => `/hosts/browser/r1-three-peer-input.test.html?peer=${peer}&line=${encodeURIComponent(lineUrl)}`;
  await pageA.goto(pageUrl("browser-a"));
  await expect(pageA.getByRole("status")).toHaveText("ready");
  await pageB.goto(pageUrl("browser-b"));
  await expect(pageB.getByRole("status")).toHaveText("ready");

  server.stdin.write("down\nup\n");
  for (const page of [pageA, pageB]) {
    const control = page.getByRole("button", { name: "Hold to control LED" });
    await control.focus();
    await page.keyboard.down("Space");
    await page.keyboard.up("Space");
    await expect(page.getByRole("status")).toHaveText("complete");
  }
  expect(await pageA.evaluate(() => globalThis.__r1InputPeer.proof())).toEqual({
    peer: "browser-a",
    sent: 2,
    acknowledgements: [
      { mergedSequence: 2, level: true },
      { mergedSequence: 3, level: false },
    ],
  });
  expect(await pageB.evaluate(() => globalThis.__r1InputPeer.proof())).toEqual({
    peer: "browser-b",
    sent: 2,
    acknowledgements: [
      { mergedSequence: 4, level: true },
      { mergedSequence: 5, level: false },
    ],
  });
  await expect.poll(() => server.exitCode).toBe(0);
  const signs = output
    .split("\n")
    .filter((line) => line.startsWith("{"))
    .map((line) => JSON.parse(line));
  expect(signs).toHaveLength(6);
  expect(signs.map(({ peer, input, requested_level: level }) => [peer, input, level])).toEqual([
    ["terminal", "keydown", true],
    ["terminal", "keyup", false],
    ["browser-a", "keydown", true],
    ["browser-a", "keyup", false],
    ["browser-b", "keydown", true],
    ["browser-b", "keyup", false],
  ]);
  expect(signs.map(({ merged_sequence: sequence }) => sequence)).toEqual([0, 1, 2, 3, 4, 5]);
  expect(signs.every(({ physical_led_result: result }) => result === null)).toBe(true);
  expect(output).toMatch(/r1-three-peer-input-complete plan=[0-9a-f]+ input_events=6 physical_led_claim=false/);
  await context.close();
});
