import { expect, test } from "@playwright/test";
import { spawn } from "node:child_process";
import { createInterface } from "node:readline";

test("the generic WebRTC DataChannel interoperates between browser and native std", async ({ page }) => {
  const native = spawn("target/debug/native-webrtc-line-probe", [], {
    cwd: new URL("../..", import.meta.url).pathname,
    stdio: ["pipe", "pipe", "pipe"],
  });
  const nativeExit = new Promise((resolve) => native.once("exit", resolve));
  let errors = "";
  native.stderr.on("data", (chunk) => { errors += chunk.toString(); });
  const lines = createInterface({ input: native.stdout });
  const responses = [];
  lines.on("line", (line) => responses.push(JSON.parse(line)));

  const offer = await page.evaluate(async () => {
    const peer = new RTCPeerConnection({ iceServers: [] });
    const channel = peer.createDataChannel("conduit-line", { ordered: true });
    globalThis.proofPeer = peer;
    globalThis.proofChannel = channel;
    await peer.setLocalDescription(await peer.createOffer());
    if (peer.iceGatheringState !== "complete") {
      await new Promise((resolve) => peer.addEventListener("icegatheringstatechange", () => {
        if (peer.iceGatheringState === "complete") resolve();
      }));
    }
    return peer.localDescription.sdp;
  });
  native.stdin.end(`${JSON.stringify({ sdp: offer })}\n`);
  await expect.poll(() => responses[0]?.kind ?? errors, { timeout: 10_000 }).toBe("answer");
  let echoed;
  try {
    echoed = await page.evaluate(async (sdp) => {
    await globalThis.proofPeer.setRemoteDescription({ type: "answer", sdp });
    await new Promise((resolve, reject) => {
      const timeout = setTimeout(() => reject(new Error("DataChannel open timed out")), 10_000);
      globalThis.proofChannel.addEventListener("open", () => { clearTimeout(timeout); resolve(); }, { once: true });
    });
    const reply = new Promise((resolve, reject) => {
      const timeout = setTimeout(() => reject(new Error("native echo timed out")), 10_000);
      globalThis.proofChannel.addEventListener("message", (event) => {
        clearTimeout(timeout);
        const bytes = [...new Uint8Array(event.data)];
        globalThis.proofChannel.send(new TextEncoder().encode("delivered"));
        resolve(bytes);
      }, { once: true });
    });
    globalThis.proofChannel.binaryType = "arraybuffer";
    globalThis.proofChannel.send(Uint8Array.from([0x43, 0x4e, 0x44, 0x57, 1]));
    return reply;
    }, responses[0].sdp);
  } catch (error) {
    throw new Error(`${error.message}\nnative stderr: ${errors}\nnative responses: ${JSON.stringify(responses)}`);
  }
  expect(echoed).toEqual([0x43, 0x4e, 0x44, 0x57, 1]);
  await expect.poll(() => responses[1]?.disposition ?? errors, { timeout: 10_000 })
    .toBe("line-ready-value-echoed");
  expect(responses[1]).toMatchObject({
    implementation_id: "std/webrtc-datachannel@1",
    received_bytes: 5,
  });
  await page.evaluate(() => globalThis.proofPeer.close());
  const exit = await Promise.race([
    nativeExit,
    new Promise((resolve) => setTimeout(() => resolve("timeout"), 5_000)),
  ]);
  if (exit === "timeout") native.kill("SIGKILL");
  expect(exit, errors).toBe(0);
});
