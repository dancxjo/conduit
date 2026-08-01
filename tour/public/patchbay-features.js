const configuredFeatures = window.CONDUIT_PATCHBAY_FEATURES || {};

/** Presentation feature flags for the Patchbay renderer. */
export const patchbayFeatures = Object.freeze({
  legacyLinePlacement: configuredFeatures.legacyLinePlacement === true
});
