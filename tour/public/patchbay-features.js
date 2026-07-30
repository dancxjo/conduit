const configuredFeatures = window.CONDUIT_PATCHBAY_FEATURES || {};

/**
 * Legacy edge routing and manual placement are retained temporarily for
 * comparison, but must not enter the normal Patchbay rendering path.
 */
export const patchbayFeatures = Object.freeze({
  legacyLinePlacement: configuredFeatures.legacyLinePlacement === true
});

