import { Conduit } from "/target/field-station-sdk/browser-sdk.mjs";
import { renderBodyPlanInspection } from "/forms/patchbay/workbench/browser/body-plan-inspection.js";

const parameters = new URLSearchParams(location.search);
const invitation = parameters.get("body");
const status = document.querySelector("#status");

try {
  if (!invitation) throw new Error("Body invitation is required");
  const host = await Conduit.browser({
    root: document.querySelector("#conduit"),
    durable: false,
  });
  let biography = null;
  let offer = null;
  const participation = await host.participate({
    invitation,
    onBiographyEvidence: (evidence) => { biography = evidence; },
    onOfferEvidence: (evidence) => { offer = evidence; },
  });
  let preparation = null;
  let play = null;
  let completion = null;
  let dispatchFailure = null;
  status.textContent = "Browser Host participating";
  globalThis.__conduitSdkParticipation = Object.freeze({
    host,
    participation,
    evidence: () => Object.freeze({ biography, offer }),
    executionCapabilities: () => participation.executionCapabilities(),
    prepare(proposal) {
      preparation = participation.prepare({
        proposal,
        inputTarget: document.querySelector("#external-body-input"),
        outputRoot: document.querySelector("#external-body-output"),
      });
      renderBodyPlanInspection(document.querySelector("#body-plan-inspection"), preparation.inspection());
      return preparation.observations();
    },
    planInspection: () => preparation.inspection(),
    start(identity) {
      const started = preparation.start(identity);
      play = started.play;
      return Object.freeze({ wakeAtStart: started.wakeAtStart, play: play.identity });
    },
    begin() {
      completion = play.dispatch().catch((error) => { dispatchFailure = error; });
    },
    cancel: () => play.terminate(),
    dispatchState: () => Object.freeze({ settled: completion !== null && play.state !== "playing", failure: dispatchFailure?.message ?? null }),
  });
} catch (error) {
  status.textContent = error instanceof Error ? error.message : String(error);
  status.dataset.failed = "";
  globalThis.__conduitSdkParticipationFailure = status.textContent;
}
