import { Conduit } from "/target/field-station-sdk/browser-sdk.mjs";

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
  status.textContent = "Browser Host participating";
  globalThis.__conduitSdkParticipation = Object.freeze({
    host,
    participation,
    evidence: () => Object.freeze({ biography, offer }),
  });
} catch (error) {
  status.textContent = error instanceof Error ? error.message : String(error);
  status.dataset.failed = "";
  globalThis.__conduitSdkParticipationFailure = status.textContent;
}
