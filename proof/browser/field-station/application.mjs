import { Conduit } from "/target/field-station-sdk/browser-sdk.mjs";

const root = document.querySelector("#conduit");
const status = document.querySelector("#status");
const identities = document.querySelector("#identities");
const lull = document.querySelector("#lull");
const patchbayRoot = document.querySelector("#patchbay");

try {
  const sourceResponse = await fetch("/forms/clock/main.conduit");
  if (!sourceResponse.ok) throw new Error("canonical Clock Form is unavailable");
  const host = await Conduit.browser({
    root,
  });
  const checked = await host.form(await sourceResponse.text()).check();
  if (!checked.ok || checked.forms.length !== 1) {
    throw new Error(checked.refusal?.message ?? "canonical Clock Form was refused");
  }
  const recoveredBody = await host.recover();
  const recoverySnapshot = recoveredBody ? await recoveredBody.current() : null;
  const body = recoveredBody ?? await host.birth({ name: "Field Station Clock", forms: checked.forms });
  const play = await body.wake();
  const patchbay = await body.patchbay({ root: patchbayRoot });
  const exact = Object.freeze({
    hostId: host.id,
    bootId: host.bootId,
    bodyId: body.id,
    wakeId: play.plan.wakeId,
    planId: play.plan.planId,
    playId: play.id,
  });
  for (const [name, value] of Object.entries({
    host: exact.hostId, boot: exact.bootId, body: exact.bodyId,
    wake: exact.wakeId, plan: exact.planId, play: exact.playId,
  })) document.querySelector(`[data-identity="${name}"]`).textContent = value;
  identities.hidden = false;
  lull.disabled = false;
  status.textContent = "Clock Body awake";

  const station = {
    host,
    body,
    play,
    patchbay,
    recovered: recoveredBody !== null,
    recoverySnapshot,
    identities: exact,
    lullSnapshot: null,
    async lull() {
      if (this.lullSnapshot) return this.lullSnapshot;
      this.lullSnapshot = await body.lull();
      lull.disabled = true;
      status.textContent = "Clock Body lulled";
      return this.lullSnapshot;
    },
  };
  lull.addEventListener("click", () => station.lull().catch(fail), { once: true });
  globalThis.__conduitFieldStation = station;
} catch (error) {
  fail(error);
}

function fail(error) {
  status.textContent = error instanceof Error ? error.message : String(error);
  status.dataset.failed = "";
  globalThis.__conduitFieldStationFailure = status.textContent;
}
