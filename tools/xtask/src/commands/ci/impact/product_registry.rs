#[derive(Debug)]
pub(super) struct ProductProofSpec {
    pub(super) id: &'static str,
    pub(super) exact_inputs: &'static [&'static str],
    pub(super) input_prefixes: &'static [&'static str],
}

impl ProductProofSpec {
    fn owns(&self, path: &str) -> bool {
        if super::is_rust_test_source(path) {
            return false;
        }
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
        ".github/workflows/tour-products.yml",
        ".github/workflows/tour-pages-deploy.yml",
        "proof/browser/package.json",
        "proof/browser/package-lock.json",
        "targets/browser/tools/build-browser-application-package.mjs",
        "targets/browser/tools/render-product-masthead.mjs",
        "tools/ci/seal-pages-carrier.mjs",
        "tools/ci/verify-pages-carrier.mjs",
        "proof/browser/executable-tour.spec.mjs",
        "proof/browser/browser-application-package.spec.mjs",
        "proof/browser/browser-bundle-build.spec.mjs",
        "proof/browser/browser-boot-profile.spec.mjs",
        "proof/browser/browser-form-runner.spec.mjs",
        "proof/browser/pages-front-door.spec.mjs",
        "proof/ci/browser-product-ownership.test.mjs",
        "proof/browser/presentation-nucleus.spec.mjs",
        "proof/browser/presentation-nucleus.test.html",
        "proof/browser/tour-test-server.mjs",
        "proof/browser/playwright.config.mjs",
        "proof/browser/static-server.mjs",
        "proof/browser/creche-browser-configuration.spec.mjs",
        "proof/browser/workspace-arrival.spec.mjs",
        "proof/browser/workspace-birth-naming.spec.mjs",
        "proof/browser/sdk-external-body-execution.spec.mjs",
        "proof/browser/workspace-mixed-membership.spec.mjs",
        "proof/browser/creche-lifecycle-ownership.md",
        "proof/browser/workspace-library.spec.mjs",
        "proof/browser/workspace-handoff.test.mjs",
        "proof/browser/creche-workspace-continuity.spec.mjs",
    ],
    input_prefixes: &[
        "products/tour/",
        "products/creche/",
        "products/workspace/",
        "products/patchbay/html/",
        "products/patchbay/model/",
        "products/patchbay/tools/",
        "products/shared/browser/",
        "semantics/presentation/assets/",
        "site/",
        "targets/browser/host/",
        "targets/browser/runtime/",
        "targets/avr/deployment/browser/",
        "targets/rp2040/deployment/browser/",
        "targets/esp32/deployment/browser/",
        "targets/std/deployment/browser/",
        "targets/browser/deployment/browser/",
        "targets/orange-pi/deployment/browser/",
        "targets/raspberry-pi/deployment/browser/",
        "targets/conduitos/deployment/browser/",
    ],
}];

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
            "targets/browser/tools/build-browser-application-package.mjs",
            "targets/browser/tools/render-product-masthead.mjs",
        ],
        input_prefixes: &[
            "site/",
            "products/shared/browser/",
            "products/tour/browser/",
            "products/creche/browser/",
            "products/workspace/browser/",
            "products/tour/tools/stage-tour-product",
            "products/creche/tools/stage-creche-product",
            "products/workspace/tools/stage-workspace-product",
            "site/tools/stage-pages-root",
            "products/patchbay/tools/stage-patchbay-product",
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
    fn birth_and_arrival_contract_changes_select_the_product_carrier_proof() {
        let root = crate::workspace::workspace_root().unwrap();
        let packages = super::super::discover(&root).unwrap();
        for path in [
            "proof/browser/workspace-arrival.spec.mjs",
            "proof/browser/workspace-birth-naming.spec.mjs",
            "proof/browser/sdk-external-body-execution.spec.mjs",
            "proof/browser/workspace-mixed-membership.spec.mjs",
            "proof/browser/workspace-library.spec.mjs",
            "proof/browser/workspace-handoff.test.mjs",
            "proof/browser/creche-workspace-continuity.spec.mjs",
        ] {
            assert_eq!(
                super::proofs_for_paths(&[path.into()]),
                ["products.pages-carrier"],
                "{path}"
            );
            let plan = super::super::plan_for_paths(&root, vec![path.into()], &packages).unwrap();
            assert!(plan.pages_products_required, "{path}");
            assert!(plan.browser_required, "{path}");
            assert!(!plan.full_fallback, "{path}");
        }

        let ownership_record = "proof/browser/creche-lifecycle-ownership.md";
        assert_eq!(
            super::proofs_for_paths(&[ownership_record.into()]),
            ["products.pages-carrier"]
        );
        let plan =
            super::super::plan_for_paths(&root, vec![ownership_record.into()], &packages).unwrap();
        assert!(plan.pages_products_required);
        assert!(!plan.browser_required);
        assert!(!plan.full_fallback);
    }

    #[test]
    fn shared_presentation_contract_changes_select_the_product_carrier() {
        for path in [
            "proof/browser/presentation-nucleus.spec.mjs",
            "proof/browser/presentation-nucleus.test.html",
            "proof/browser/tour-test-server.mjs",
            "proof/browser/playwright.config.mjs",
            "proof/browser/static-server.mjs",
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
            "products/shared/browser/conduit.css",
            "products/tour/browser/tour.mjs",
            "products/workspace/browser/body-bootstrap.mjs",
            "products/workspace/browser/reviewed-form-selection.mjs",
        ] {
            assert!(proofs_for_paths(&[path.to_owned()]).contains(&"products.pages-carrier"));
            assert!(!browser_presentation_proofs_for_path(path).is_empty());
        }
    }

    #[test]
    fn broad_product_and_site_roots_own_their_carrier_staging_tools() {
        for path in [
            "products/tour/tools/stage-tour-product.mjs",
            "products/creche/tools/stage-creche-product.mjs",
            "products/workspace/tools/stage-workspace-product.mjs",
            "products/patchbay/tools/stage-patchbay-product.mjs",
            "site/tools/stage-pages-root.mjs",
        ] {
            assert_eq!(
                proofs_for_paths(&[path.to_owned()]),
                ["products.pages-carrier"]
            );
        }
    }

    #[test]
    fn rust_test_sources_do_not_fabricate_the_product_carrier() {
        for path in [
            "targets/browser/runtime/src/workspace_mask_tests.rs",
            "targets/browser/runtime/src/form_runner/tests.rs",
            "targets/browser/runtime/tests/presentation_offer_ownership.rs",
            "products/patchbay/model/src/mask_control_tests.rs",
            "products/patchbay/html/tests/server.rs",
        ] {
            assert!(proofs_for_paths(&[path.to_owned()]).is_empty(), "{path}");
        }
    }

    #[test]
    fn native_semantics_do_not_select_the_browser_carrier() {
        for path in [
            "products/patchbay/native/Cargo.toml",
            "products/patchbay/native/src/gui.rs",
            "products/home/model/src/lib.rs",
        ] {
            assert!(proofs_for_paths(&[path.to_owned()]).is_empty(), "{path}");
            assert!(
                browser_presentation_proofs_for_path(path).is_empty(),
                "{path}"
            );
        }
    }

    #[test]
    fn directly_staged_target_adapters_require_the_product_carrier() {
        for target in [
            "avr",
            "rp2040",
            "esp32",
            "std",
            "browser",
            "orange-pi",
            "raspberry-pi",
            "conduitos",
        ] {
            let path = format!("targets/{target}/deployment/browser/creche-adapter.mjs");
            assert_eq!(
                proofs_for_paths(std::slice::from_ref(&path)),
                ["products.pages-carrier"],
                "{path}"
            );
        }
    }

    #[test]
    fn machine_and_fabrication_sources_do_not_select_the_browser_carrier() {
        for path in [
            "targets/conduitos/src/main.rs",
            "targets/conduitos/fabrication/xtask/journey_body_track.rs",
            "targets/esp32/firmware/s3-signal/build.rs",
            "targets/esp32/fabrication/src/lib.rs",
            "targets/avr/firmware/promicro-host/Cargo.toml",
            "targets/avr/fabrication/src/lib.rs",
            "targets/raspberry-pi/fabrication/src/lib.rs",
            "targets/orange-pi/fabrication/src/lib.rs",
            "targets/std/fabrication/src/lib.rs",
            "targets/std/profiles/linux-computer.host.conduit",
            "targets/browser/profiles/browser-page.host.conduit",
            "targets/rp2040/profiles/pico-w.host.conduit",
            "fabrication/host/src/lib.rs",
            "fabrication/workspace/src/lib.rs",
        ] {
            assert!(proofs_for_paths(&[path.into()]).is_empty(), "{path}");
        }
    }
}
