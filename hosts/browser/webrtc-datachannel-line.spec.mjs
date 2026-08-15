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
    left.goto("/hosts/browser/webrtc-datachannel-line.test.html"),
    right.goto("/hosts/browser/webrtc-datachannel-line.test.html"),
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
  expect(state.channel.retainedMessages).toBe(1);
  expect(state.channel.retainedBytes).toBe(1);
});
