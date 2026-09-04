#[derive(Debug)]
pub(super) struct ProductProofSpec {
    pub(super) id: &'static str,
    pub(super) exact_inputs: &'static [&'static str],
    pub(super) input_prefixes: &'static [&'static str],
}

impl ProductProofSpec {
    fn owns(&self, path: &str) -> bool {
        self.exact_inputs.contains(&path)
            || self
                .input_prefixes
                .iter()
                .any(|prefix| path.starts_with(prefix))
    }
}

// This registry is the single ownership source for the Pages product carrier.
// Workflow triggers must not duplicate these paths. Unknown global changes are
// still handled by the impact planner's conservative full fallback.
pub(super) const PRODUCT_PROOFS: &[ProductProofSpec] = &[ProductProofSpec {
    id: "products.pages-carrier",
    exact_inputs: &[
        ".github/workflows/executable-book-pages.yml",
        ".github/workflows/executable-book-deploy.yml",
        "Cargo.lock",
        "package.json",
        "package-lock.json",
        "scripts/ci/build-browser-application-package.mjs",
        "scripts/ci/seal-pages-carrier.mjs",
        "scripts/ci/verify-pages-carrier.mjs",
        "proof/browser/executable-book.spec.mjs",
        "proof/browser/browser-application-package.spec.mjs",
        "proof/browser/browser-bundle-build.spec.mjs",
        "proof/browser/browser-boot-profile.spec.mjs",
        "proof/browser/browser-form-runner.spec.mjs",
        "proof/browser/pages-front-door.spec.mjs",
        "proof/browser/creche-browser-configuration.spec.mjs",
    ],
    input_prefixes: &[
        "products/tour/",
        "products/patchbay/",
        "site/",
        "scripts/ci/stage-book-product",
        "scripts/ci/stage-creche-product",
        "scripts/ci/stage-pages-root",
        "scripts/ci/stage-patchbay-product",
        "targets/browser/host/",
        "targets/browser/runtime/",
        "targets/avr/",
        "targets/esp32/",
        "targets/orange-pi/",
        "targets/raspberry-pi/",
        "targets/std/browser-deployment/",
        "targets/std/fabrication-package/",
        "targets/conduitos/",
        "profiles/host-configurations/",
        "profiles/hosts/",
        "fabrication/host/",
        "fabrication/workspace/",
    ],
}];

pub(super) fn proofs_for_paths(paths: &[String]) -> Vec<&'static str> {
    PRODUCT_PROOFS
        .iter()
        .filter(|spec| paths.iter().any(|path| spec.owns(path)))
        .map(|spec| spec.id)
        .collect()
}
