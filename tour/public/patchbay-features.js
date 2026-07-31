const configuredFeatures = window.CONDUIT_PATCHBAY_FEATURES || {};

/** Presentation feature flags for the Patchbay renderer. Routing itself is
 * selected by the hybrid edge from measured layout and native Bézier safety. */
export const patchbayFeatures = Object.freeze({
  legacyLinePlacement: configuredFeatures.legacyLinePlacement === true
});
