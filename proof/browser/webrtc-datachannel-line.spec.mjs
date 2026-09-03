import { expect, test } from "@playwright/test";

const limits = {
  maximumMessageBytes: 8,
  maximumBufferedBytes: 8,
  maximumReceivedMessages: 2,
};

async function connect(context, leftLimits = limits, rightLimits = limits) {
  const left = await context.newPage();
  const right = await context.newPage();
  await Promise.all([
    left.goto("/proof/browser/webrtc-datachannel-line.test.html"),
    right.goto("/proof/browser/webrtc-datachannel-line.test.html"),
  ]);
  const offer = await left.evaluate((value) => window.conduitOffer(value), leftLimits);
  const answer = await right.evaluate(
    ({ offer, limits: value }) => window.conduitAnswer(offer, value),
    { offer, limits: rightLimits },
  );
  await left.evaluate((value) => window.conduitAcceptAnswer(value), answer);
  await Promise.all([
    left.evaluate(() => window.conduitOpen()),
    right.evaluate(() => window.conduitOpen()),
  ]);
  return { left, right };
}

const sessionLimits = {
  maximumMessageBytes: 1024,
  maximumBufferedBytes: 2048,
  maximumReceivedMessages: 1,
};

async function transfer(sender, receiver, bytes) {
  expect(await sender.evaluate((value) => window.conduitSend([value]), bytes)).toEqual([
    { accepted: true },
  ]);
  const received = await receiver.evaluate(() => window.conduitReceive());
  expect(received.ok).toBe(true);
  expect(received.bytes).toEqual(bytes);
  return receiver.evaluate((value) => window.conduitSessionIngest(value), received.bytes);
}

async function transferWithPressure(sender, receiver, bytes) {
  expect(await sender.evaluate((value) => window.conduitSend([value]), bytes)).toEqual([
    { accepted: true },
  ]);
  const received = await receiver.evaluate(() => window.conduitReceive());
  expect(received).toEqual({ ok: true, bytes });
  return receiver.evaluate(
    (value) => window.conduitSessionPressure(value),
    received.bytes,
  );
}

async function activeSession(context) {
  const { left, right } = await connect(context, sessionLimits, sessionLimits);
  expect(await left.evaluate(() => window.conduitSessionStart(0))).toBe(0);
  expect(await right.evaluate(() => window.conduitSessionStart(1))).toBe(0);
  const leftHello = await left.evaluate(() => window.conduitSessionOutput());
  const rightHello = await right.evaluate(() => window.conduitSessionOutput());
  expect(await transfer(left, right, leftHello)).toBe(0);
  expect(await transfer(right, left, rightHello)).toBe(0);
  const leftReady = await left.evaluate(() => window.conduitSessionOutput());
  const rightReady = await right.evaluate(() => window.conduitSessionOutput());
  expect(await transfer(left, right, leftReady)).toBe(1);
  expect(await transfer(right, left, rightReady)).toBe(1);
  return { left, right, leftReady, rightReady };
}

test("reviewed DataChannel bounds refuse oversized session configuration", async ({ context }) => {
  await expect(connect(context, {
    maximumMessageBytes: 129 * 1024,
    maximumBufferedBytes: 256 * 1024,
    maximumReceivedMessages: 1,
  })).rejects.toThrow("maximumMessageBytes exceeds the reviewed Line bound");
  await expect(connect(context, {
    maximumMessageBytes: 1024,
    maximumBufferedBytes: 257 * 1024,
    maximumReceivedMessages: 1,
  })).rejects.toThrow("maximumBufferedBytes exceeds the reviewed Line bound");
  await expect(connect(context, {
    maximumMessageBytes: 1024,
    maximumBufferedBytes: 2048,
    maximumReceivedMessages: 17,
  })).rejects.toThrow("maximumReceivedMessages exceeds the reviewed Line bound");
});

test("two browser Hosts exchange bounded binary DataChannel messages then observe Line loss", async ({
  context,
}) => {
  const { left, right } = await connect(context);
  expect(await left.evaluate(() => window.conduitSend([[1, 2, 3]]))).toEqual([
    { accepted: true },
  ]);
  expect(await right.evaluate(() => window.conduitReceive())).toEqual({
    ok: true,
    bytes: [1, 2, 3],
  });
  expect(await right.evaluate(() => window.conduitSend([[4, 5]]))).toEqual([
    { accepted: true },
  ]);
  expect(await left.evaluate(() => window.conduitReceive())).toEqual({
    ok: true,
    bytes: [4, 5],
  });
  expect(await left.evaluate(() => window.conduitSend([
    [0, 1, 2, 3, 4, 5, 6, 7],
    [8, 9, 10, 11, 12, 13, 14, 15],
  ]))).toEqual([
    { accepted: true },
    { accepted: false, reason: "buffer-pressure" },
  ]);
  expect(await right.evaluate(() => window.conduitReceive())).toEqual({
    ok: true,
    bytes: [0, 1, 2, 3, 4, 5, 6, 7],
  });
  expect(await left.evaluate(() => window.conduitSend([[0, 1, 2, 3, 4, 5, 6, 7, 8]])))
    .toEqual([{ accepted: false, reason: "message-too-large" }]);

  await left.evaluate(() => window.conduitClose());
  expect(await right.evaluate(() => window.conduitClosed())).toEqual({
    ok: false,
    reason: "closed",
  });
  const rightState = await right.evaluate(() => window.conduitState());
  expect(rightState.channel.readyState).toBe("closed");
  expect(["connected", "connecting"]).toContain(rightState.peer);
});

test("receive oversize and queue pressure fail closed without unbounded retention", async ({
  context,
}) => {
  const oversize = await connect(context, limits, {
    ...limits,
    maximumMessageBytes: 4,
  });
  expect(await oversize.left.evaluate(() => window.conduitSend([[1, 2, 3, 4, 5]])))
    .toEqual([{ accepted: true }]);
  expect(await oversize.right.evaluate(() => window.conduitClosed())).toEqual({
    ok: false,
    reason: "message-too-large",
  });
  expect(await oversize.right.evaluate(() => window.conduitOpen().then(
    () => "opened",
    (error) => error.message,
  ))).toBe("datachannel-terminal:message-too-large");
  expect(await oversize.right.evaluate(() => window.conduitSend([[9]]))).toEqual([{
    accepted: false,
    reason: "line-terminal",
    terminalReason: "message-too-large",
  }]);
  expect(await oversize.right.evaluate(() => window.conduitReceive())).toEqual({
    ok: false,
    reason: "message-too-large",
  });
  expect((await oversize.right.evaluate(() => window.conduitState())).channel)
    .toMatchObject({
      retainedMessages: 0,
      retainedBytes: 0,
      terminalReason: "message-too-large",
    });

  const pressure = await connect(context, limits, {
    maximumMessageBytes: 4,
    maximumBufferedBytes: 8,
    maximumReceivedMessages: 1,
  });
  expect(await pressure.left.evaluate(() => window.conduitSend([[1], [2]]))).toEqual([
    { accepted: true },
    { accepted: true },
  ]);
  expect(await pressure.right.evaluate(() => window.conduitClosed())).toEqual({
    ok: false,
    reason: "receive-pressure",
  });
  const state = await pressure.right.evaluate(() => window.conduitState());
  expect(state.channel.retainedMessages).toBe(0);
  expect(state.channel.retainedBytes).toBe(0);
  expect(state.channel.terminalReason).toBe("receive-pressure");
  expect(await pressure.right.evaluate(() => window.conduitSend([[3]]))).toEqual([{
    accepted: false,
    reason: "line-terminal",
    terminalReason: "receive-pressure",
  }]);
  expect(await pressure.right.evaluate(() => window.conduitOpen().then(
    () => "opened",
    (error) => error.message,
  ))).toBe("datachannel-terminal:receive-pressure");
});

test("terminal transport error fences a still-open raw channel and keeps first reason", async ({
  context,
}) => {
  const { left } = await connect(context);
  const outcome = await left.evaluate(() => {
    const original = RTCDataChannel.prototype.send;
    RTCDataChannel.prototype.send = function throwSend() {
      throw new Error("deterministic send exception");
    };
    try {
      return {
        first: window.conduitSend([[1]]),
        rawAfterException: window.conduitState().channel.readyState,
        second: window.conduitSend([[2]]),
      };
    } finally {
      RTCDataChannel.prototype.send = original;
    }
  });
  expect(outcome).toEqual({
    first: [{ accepted: false, reason: "transport-error" }],
    rawAfterException: "open",
    second: [{
      accepted: false,
      reason: "line-terminal",
      terminalReason: "transport-error",
    }],
  });
  expect(await left.evaluate(() => window.conduitClosed())).toEqual({
    ok: false,
    reason: "transport-error",
  });
  expect(await left.evaluate(() => window.conduitOpen().then(
    () => "opened",
    (error) => error.message,
  ))).toBe("datachannel-terminal:transport-error");
  await left.evaluate(() => window.conduitClose());
  expect(await left.evaluate(() => window.conduitClosed())).toEqual({
    ok: false,
    reason: "transport-error",
  });
  expect((await left.evaluate(() => window.conduitState())).channel.terminalReason)
    .toBe("transport-error");
});

test("exact planned WebRTC session admits, terminates, and rejects late frames", async ({
  context,
}) => {
  const { left, right, leftReady } = await activeSession(context);
  expect(await left.evaluate(() => window.conduitSessionCloseInput())).toBe(1);
  const inputClosed = await left.evaluate(() => window.conduitSessionOutput());
  expect(await transfer(left, right, inputClosed)).toBe(1);
  expect(await right.evaluate(() => window.conduitSessionFinish())).toBe(3);
  const rightTerminal = await right.evaluate(() => window.conduitSessionOutput());
  expect(await transfer(right, left, rightTerminal)).toBe(3);
  expect(await left.evaluate(() => window.conduitSessionFinish())).toBe(2);
  const leftTerminal = await left.evaluate(() => window.conduitSessionOutput());
  expect(await transfer(left, right, leftTerminal)).toBe(2);
  expect(await left.evaluate(
    (bytes) => window.conduitSessionIngest(bytes),
    leftReady,
  )).toBe(-216);
});

test("Body-granted canonical Hello initializes the exact browser session", async ({ page }) => {
  await page.goto("/proof/browser/webrtc-datachannel-line.test.html");
  expect(await page.evaluate(() => window.conduitSessionStart(0))).toBe(0);
  const grantedHello = await page.evaluate(() => window.conduitSessionOutput());
  expect(await page.evaluate(
    (hello) => window.conduitSessionStartGranted("source", hello),
    grantedHello,
  )).toBe(0);
  expect(await page.evaluate(() => window.conduitSessionOutput())).toEqual(grantedHello);
  await expect(page.evaluate(
    () => window.conduitSessionStartGranted("source", [1, 2, 3]),
  )).rejects.toThrow("granted start refused");
  const zeroIndex = grantedHello.indexOf(0);
  expect(zeroIndex).toBeGreaterThanOrEqual(0);
  const coercingHello = [...grantedHello];
  coercingHello[zeroIndex] = 256;
  await expect(page.evaluate(
    (hello) => window.conduitSessionStartGranted("source", hello),
    coercingHello,
  )).rejects.toThrow("invalid Body grant");
  await expect(page.evaluate(
    (hello) => window.conduitSessionStartGranted("fixture-variant-1", hello),
    grantedHello,
  )).rejects.toThrow("invalid Body grant");
});

test("Body grants compose one real peer with the exact shared session", async ({ context }) => {
  const left = await context.newPage();
  const right = await context.newPage();
  await Promise.all([
    left.goto("/proof/browser/webrtc-datachannel-line.test.html"),
    right.goto("/proof/browser/webrtc-datachannel-line.test.html"),
  ]);
  await left.evaluate(() => window.conduitSessionStart(0));
  const hello = await left.evaluate(() => window.conduitSessionOutput());
  const negotiationId = "browser-webrtc/binding/1";
  await expect(right.evaluate(() => window.conduitBodySessionCreate({
    negotiation_id: "browser-webrtc/binding/1",
    role: "sink",
    peer_host_id: "browser-webrtc/source",
    peer_boot_id: "browser-webrtc/source-boot/1",
    session_hello: [],
  }))).rejects.toThrow("invalid session Hello");
  await right.evaluate(({ negotiationId, hello }) => window.conduitBodySessionCreate({
    negotiation_id: negotiationId,
    role: "sink",
    peer_host_id: "browser-webrtc/source",
    peer_boot_id: "browser-webrtc/source-boot/1",
    session_hello: hello,
  }), { negotiationId, hello });
  await expect(right.evaluate(({ negotiationId, hello }) => (
    window.conduitBodySessionCreateWithThrowingSignal({
      negotiation_id: negotiationId,
      role: "source",
      peer_host_id: "browser-webrtc/sink",
      peer_boot_id: "browser-webrtc/sink-boot/1",
      session_hello: hello,
    })
  ), { negotiationId, hello })).rejects.toThrow("signal relay failed");
  await left.evaluate(({ negotiationId, hello }) => window.conduitBodySessionCreate({
    negotiation_id: negotiationId,
    role: "source",
    peer_host_id: "browser-webrtc/sink",
    peer_boot_id: "browser-webrtc/sink-boot/1",
    session_hello: hello,
  }), { negotiationId, hello });

  const [offer] = await left.evaluate(() => window.conduitBodySessionSignals());
  expect(offer.signal).toMatchObject({ negotiation_id: negotiationId, description: "offer" });
  await expect(right.evaluate(
    (signal) => window.conduitBodySessionAccept({
      source_host_id: "browser-webrtc/source",
      source_boot_id: "browser-webrtc/source-boot/1",
      signal: { ...signal.signal, sdp: "x".repeat(4097) },
    }),
    offer,
  )).rejects.toThrow("invalid remote SDP");
  await expect(right.evaluate(
    (signal) => window.conduitBodySessionAccept({
      source_host_id: "browser-webrtc/wrong-source",
      source_boot_id: "browser-webrtc/source-boot/1",
      signal: signal.signal,
    }),
    offer,
  )).rejects.toThrow("signal identity refused");
  await right.evaluate(
    (signal) => window.conduitBodySessionAccept({
      source_host_id: "browser-webrtc/source",
      source_boot_id: "browser-webrtc/source-boot/1",
      signal: signal.signal,
    }),
    offer,
  );
  const [answer] = await right.evaluate(() => window.conduitBodySessionSignals());
  expect(answer.signal).toMatchObject({ negotiation_id: negotiationId, description: "answer" });
  await left.evaluate(
    (signal) => window.conduitBodySessionAccept({
      source_host_id: "browser-webrtc/sink",
      source_boot_id: "browser-webrtc/sink-boot/1",
      signal: signal.signal,
    }),
    answer,
  );
  const [leftReady, rightReady] = await Promise.all([
    left.evaluate(() => window.conduitBodySessionReady()),
    right.evaluate(() => window.conduitBodySessionReady()),
  ]);
  expect(leftReady).toMatchObject({
    negotiationId,
    role: "source",
    peerHostId: "browser-webrtc/sink",
    peerBootId: "browser-webrtc/sink-boot/1",
    sessionReady: true,
    terminalReason: null,
  });
  expect(rightReady).toMatchObject({
    negotiationId,
    role: "sink",
    peerHostId: "browser-webrtc/source",
    peerBootId: "browser-webrtc/source-boot/1",
    sessionReady: true,
    terminalReason: null,
  });
  await left.evaluate(() => window.conduitBodySessionClose());
  await expect.poll(() => right.evaluate(() => window.conduitBodySessionState()))
    .toMatchObject({
      peerState: "closed",
      sessionReady: false,
      terminalReason: "line-closed",
    });
});

test("shared session machine refuses mismatched planned and finite wire facts", async ({
  context,
}) => {
  const refusals = [
    { variant: 5, code: -210 },
    { variant: 3, code: -211 },
    { variant: 1, code: -212 },
    { variant: 2, code: -213 },
    { variant: 4, code: -214 },
  ];
  for (const refusal of refusals) {
    const { left, right } = await connect(context, sessionLimits, sessionLimits);
    expect(await left.evaluate(() => window.conduitSessionStart(0))).toBe(0);
    expect(await right.evaluate(
      (variant) => window.conduitSessionStart(1, variant),
      refusal.variant,
    )).toBe(0);
    const hello = await left.evaluate(() => window.conduitSessionOutput());
    await right.evaluate(() => window.conduitSessionOutput());
    expect(await transfer(left, right, hello)).toBe(refusal.code);
    await Promise.all([left.close(), right.close()]);
  }

  const malformed = await connect(context, sessionLimits, sessionLimits);
  expect(await malformed.right.evaluate(() => window.conduitSessionStart(1))).toBe(0);
  await malformed.right.evaluate(() => window.conduitSessionOutput());
  expect(await malformed.right.evaluate(
    () => window.conduitSessionIngest([1, 2, 3]),
  )).toBe(-219);
  expect(await malformed.right.evaluate(
    () => window.conduitSessionIngest(new Array((128 * 1024) + 1).fill(0)),
  )).toBe(-215);
});

test("ordinary bounded Cord value crosses canonical pressure and delivery states", async ({
  context,
}) => {
  const { left, right } = await activeSession(context);
  const value = [11, 22, 33, 44];

  expect(await left.evaluate((bytes) => window.conduitSessionOffer(bytes), value)).toBe(1);
  const pressuredOffer = await left.evaluate(() => window.conduitSessionOutput());
  expect(await transferWithPressure(left, right, pressuredOffer)).toBe(1);
  const pressure = await right.evaluate(() => window.conduitSessionOutput());
  expect(await transfer(right, left, pressure)).toBe(1);
  expect(await left.evaluate(() => window.conduitSessionNextSequence())).toBe(0);
  expect(await right.evaluate(() => window.conduitSessionNextSequence())).toBe(0);

  expect(await left.evaluate((bytes) => window.conduitSessionOffer(bytes), value)).toBe(1);
  const offered = await left.evaluate(() => window.conduitSessionOutput());
  expect(await transfer(left, right, offered)).toBe(1);
  expect(await right.evaluate(() => window.conduitSessionValue())).toEqual(value);
  expect(await left.evaluate((bytes) => window.conduitSessionOffer(bytes), [55])).toBe(-217);
  expect(await left.evaluate(
    (bytes) => window.conduitSessionOffer(bytes),
    new Array((64 * 1024) + 1).fill(1),
  )).toBe(-215);

  const accepted = await right.evaluate(() => window.conduitSessionOutput());
  expect(await right.evaluate(
    (bytes) => window.conduitSessionIngest(bytes),
    offered,
  )).toBe(-218);
  expect(await right.evaluate(() => window.conduitSessionValue())).toEqual(value);
  expect(await right.evaluate(() => window.conduitSessionNextSequence())).toBe(0);
  expect(await transfer(right, left, accepted)).toBe(1);
  expect(await left.evaluate(() => window.conduitSessionNextSequence())).toBe(0);
  expect(await right.evaluate(() => window.conduitSessionDeliver())).toBe(1);
  expect(await right.evaluate(() => window.conduitSessionValue())).toEqual([]);
  const delivered = await right.evaluate(() => window.conduitSessionOutput());
  expect(await transfer(right, left, delivered)).toBe(1);
  expect(await left.evaluate(() => window.conduitSessionNextSequence())).toBe(1);
  expect(await right.evaluate(() => window.conduitSessionNextSequence())).toBe(1);
  expect(await left.evaluate(
    (bytes) => window.conduitSessionIngest(bytes),
    delivered,
  )).toBe(-217);

  expect(await left.evaluate(() => window.conduitSessionCloseInput())).toBe(1);
  expect(await transfer(
    left,
    right,
    await left.evaluate(() => window.conduitSessionOutput()),
  )).toBe(1);
  expect(await right.evaluate(() => window.conduitSessionFinish())).toBe(3);
  expect(await transfer(
    right,
    left,
    await right.evaluate(() => window.conduitSessionOutput()),
  )).toBe(3);
  expect(await left.evaluate(() => window.conduitSessionFinish())).toBe(2);
  expect(await transfer(
    left,
    right,
    await left.evaluate(() => window.conduitSessionOutput()),
  )).toBe(2);
  expect(await left.evaluate(
    (bytes) => window.conduitSessionIngest(bytes),
    delivered,
  )).toBe(-216);
});
