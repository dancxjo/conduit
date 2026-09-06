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
pub(super) const PRODUCT_PROOFS: &[ProductProofSpec] = &[
    ProductProofSpec {
        id: "products.pages-carrier",
        exact_inputs: &[
            ".github/workflows/executable-book-pages.yml",
            ".github/workflows/executable-book-deploy.yml",
            "Cargo.lock",
            "package.json",
            "package-lock.json",
            "scripts/ci/build-browser-application-package.mjs",
            "scripts/ci/render-product-masthead.mjs",
            "scripts/ci/seal-pages-carrier.mjs",
            "scripts/ci/verify-pages-carrier.mjs",
            "proof/browser/executable-book.spec.mjs",
            "proof/browser/browser-application-package.spec.mjs",
            "proof/browser/browser-bundle-build.spec.mjs",
            "proof/browser/browser-boot-profile.spec.mjs",
            "proof/browser/browser-form-runner.spec.mjs",
            "proof/browser/pages-front-door.spec.mjs",
            "proof/ci/browser-product-ownership.test.mjs",
            "proof/browser/presentation-nucleus.spec.mjs",
            "proof/browser/presentation-nucleus.test.html",
            "proof/browser/fourth-product-conformance.spec.mjs",
            "proof/browser/book-test-server.mjs",
            "proof/browser/playwright.config.mjs",
            "proof/browser/static-server.mjs",
            "proof/browser/creche-browser-configuration.spec.mjs",
        ],
        input_prefixes: &[
            "proof/browser/fourth-product/",
            "products/tour/",
            "products/creche/",
            "products/patchbay/",
            "semantics/presentation/assets/",
            "site/",
            "products/tour/browser/",
            "products/creche/browser/",
            "scripts/ci/stage-tour-product",
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
    },
    ProductProofSpec {
        id: "products.patchbay-debugger",
        exact_inputs: &[
            ".github/workflows/executable-book-pages.yml",
            "Cargo.lock",
            "package.json",
            "package-lock.json",
            "architecture/kernel/src/debug_observation.rs",
            "architecture/kernel/src/scheduler.rs",
            "architecture/kernel/tests/debug_observation.rs",
            "proof/browser/patchbay-debugger-watch.spec.mjs",
            "proof/browser/patchbay-debugger.config.mjs",
        ],
        input_prefixes: &[
            "architecture/kernel/src/debug_observation/",
            "architecture/kernel/src/scheduler/debug_control.rs",
            "products/patchbay/html/",
            "products/patchbay/model/src/debugger_",
            "products/patchbay/model/src/learned_watch",
            "semantics/tongues/",
        ],
    },
];

pub(crate) fn proofs_for_paths(paths: &[String]) -> Vec<&'static str> {
    PRODUCT_PROOFS
        .iter()
        .filter(|spec| paths.iter().any(|path| spec.owns(path)))
        .map(|spec| spec.id)
        .collect()
}

pub(crate) fn contains(proof_id: &str) -> bool {
    PRODUCT_PROOFS.iter().any(|proof| proof.id == proof_id)
}

#[derive(Debug)]
pub(super) struct BrowserPresentationSpec {
    pub(super) id: &'static str,
    pub(super) exact_inputs: &'static [&'static str],
    pub(super) input_prefixes: &'static [&'static str],
}

impl BrowserPresentationSpec {
    fn owns(&self, path: &str) -> bool {
        self.exact_inputs.contains(&path)
            || self
                .input_prefixes
                .iter()
                .any(|prefix| path.starts_with(prefix))
    }
}

// These inputs change browser presentation or how an already-fabricated
// browser product is assembled. They require the Pages/browser product proof,
// but cannot change firmware or an operating-system image.
pub(super) const BROWSER_PRESENTATION_PROOFS: &[BrowserPresentationSpec] =
    &[BrowserPresentationSpec {
        id: "products.browser-presentation",
        exact_inputs: &[
            "scripts/ci/build-browser-application-package.mjs",
            "scripts/ci/render-product-masthead.mjs",
        ],
        input_prefixes: &[
            "site/",
            "products/tour/browser/",
            "products/creche/browser/",
            "scripts/ci/stage-tour-product",
            "scripts/ci/stage-creche-product",
            "scripts/ci/stage-pages-root",
            "scripts/ci/stage-patchbay-product",
        ],
    }];

pub(super) fn browser_presentation_proofs_for_path(
    path: &str,
) -> Vec<&'static BrowserPresentationSpec> {
    BROWSER_PRESENTATION_PROOFS
        .iter()
        .filter(|spec| spec.owns(path))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_presentation_contract_and_fixture_changes_select_the_product_carrier() {
        for path in [
            "proof/browser/presentation-nucleus.spec.mjs",
            "proof/browser/presentation-nucleus.test.html",
            "proof/browser/fourth-product-conformance.spec.mjs",
            "proof/browser/book-test-server.mjs",
            "proof/browser/playwright.config.mjs",
            "proof/browser/static-server.mjs",
            "proof/browser/fourth-product/application.mjs",
            "proof/browser/fourth-product/state.mjs",
            "proof/browser/fourth-product/fourth.application.template.json",
        ] {
            assert!(
                proofs_for_paths(&[path.to_owned()]).contains(&"products.pages-carrier"),
                "{path}"
            );
        }
    }
}

#[cfg(test)]
mod product_source_tests {
    use super::*;

    #[test]
    fn product_owned_browser_source_requires_carrier_and_browser_proof() {
        for path in [
            "products/tour/browser/tour.mjs",
            "products/creche/browser/creche-lifecycle.mjs",
        ] {
            assert!(proofs_for_paths(&[path.to_owned()]).contains(&"products.pages-carrier"));
            assert!(!browser_presentation_proofs_for_path(path).is_empty());
        }
    }
}
