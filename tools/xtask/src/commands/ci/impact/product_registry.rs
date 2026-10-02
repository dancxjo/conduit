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
        ".github/workflows/product-carrier.yml",
        ".github/workflows/pages-deploy.yml",
        "proof/browser/package.json",
        "proof/browser/package-lock.json",
        "targets/browser/tools/build-browser-application-package.mjs",
        "targets/browser/tools/render-product-masthead.mjs",
        "tools/ci/seal-pages-carrier.mjs",
        "tools/ci/verify-pages-carrier.mjs",
        "proof/browser/browser-bundle-build.spec.mjs",
        "proof/browser/browser-boot-profile.spec.mjs",
        "proof/browser/pages-front-door.spec.mjs",
        "proof/ci/browser-product-ownership.test.mjs",
        "proof/browser/presentation-nucleus.spec.mjs",
        "proof/browser/presentation-nucleus.test.html",
        "proof/browser/static-product-server.mjs",
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
        "proof/browser/workspace-continuity.spec.mjs",
    ],
    input_prefixes: &[
        "docs/journeys/tour/",
        "targets/browser/workspace/",
        "targets/browser/patchbay-workbench/",
        "plots/patchbay/workbench/model/",
        "targets/browser/tools/stage-browser-workspace",
        "targets/browser/tools/stage-patchbay-workbench",
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

// These inputs change browser presentation or how an already-made
// browser product is assembled. They require the Pages/browser product proof,
// but cannot change firmware or an operating-system image.
pub(super) const BROWSER_PRESENTATION_PROOFS: &[BrowserPresentationSpec] =
    &[BrowserPresentationSpec {
        id: "products.browser-presentation",
        exact_inputs: &[
            "targets/browser/tools/build-browser-application-package.mjs",
            "targets/browser/tools/render-product-masthead.mjs",
            "targets/browser/host/assets/conduit.css",
        ],
        input_prefixes: &[
            "site/",
            "docs/journeys/tour/",
            "targets/browser/workspace/",
            "targets/browser/tools/stage-browser-workspace",
            "site/tools/stage-pages-root",
            "targets/browser/tools/stage-patchbay-workbench",
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
            "proof/browser/workspace-continuity.spec.mjs",
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
            "proof/browser/static-product-server.mjs",
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
    fn browser_presentation_source_requires_carrier_and_browser_proof() {
        for path in [
            "targets/browser/host/assets/conduit.css",
            "docs/journeys/tour/chapter-1.md",
            "targets/browser/workspace/body-bootstrap.mjs",
            "targets/browser/workspace/reviewed-plot-selection.mjs",
        ] {
            assert!(proofs_for_paths(&[path.to_owned()]).contains(&"products.pages-carrier"));
            assert!(!browser_presentation_proofs_for_path(path).is_empty());
        }
    }

    #[test]
    fn browser_target_and_site_roots_own_their_carrier_staging_tools() {
        for path in [
            "targets/browser/tools/stage-browser-workspace.sh",
            "targets/browser/tools/stage-patchbay-workbench.sh",
            "site/tools/stage-pages-root.mjs",
        ] {
            assert_eq!(
                proofs_for_paths(&[path.to_owned()]),
                ["products.pages-carrier"]
            );
        }
    }

    #[test]
    fn rust_test_sources_do_not_make_the_product_carrier() {
        for path in [
            "targets/browser/runtime/src/workspace_mask_tests.rs",
            "targets/browser/runtime/src/plot_runner/tests.rs",
            "targets/browser/runtime/tests/presentation_offer_ownership.rs",
            "plots/patchbay/workbench/model/src/mask_control_tests.rs",
            "targets/browser/patchbay-workbench/tests/server.rs",
        ] {
            assert!(proofs_for_paths(&[path.to_owned()]).is_empty(), "{path}");
        }
    }

    #[test]
    fn native_semantics_do_not_select_the_browser_carrier() {
        for path in ["semantics/home/src/lib.rs", "targets/conduitos/src/main.rs"] {
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
    fn machine_and_make_sources_do_not_select_the_browser_carrier() {
        for path in [
            "targets/conduitos/src/main.rs",
            "targets/conduitos/make/xtask/journey_body_track.rs",
            "targets/esp32/firmware/s3-signal/build.rs",
            "targets/esp32/make/src/lib.rs",
            "targets/avr/firmware/promicro-host/Cargo.toml",
            "targets/avr/make/src/lib.rs",
            "targets/raspberry-pi/make/src/lib.rs",
            "targets/orange-pi/make/src/lib.rs",
            "targets/std/make/src/lib.rs",
            "targets/std/profiles/linux-computer.host.conduit",
            "targets/browser/profiles/browser-page.host.conduit",
            "targets/rp2040/profiles/pico-w.host.conduit",
            "make/host/src/lib.rs",
            "make/workspace/src/lib.rs",
        ] {
            assert!(proofs_for_paths(&[path.into()]).is_empty(), "{path}");
        }
    }
}
