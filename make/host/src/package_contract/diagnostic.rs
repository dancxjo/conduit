//! Refusals for checked Make package composition.
use crate::SporeOutputKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageCompositionDiagnostic {
    InitializationContract {
        package_id: String,
        implementation_id: String,
        detail: String,
    },
    DuplicatePackageIdentity {
        package_id: String,
    },
    DuplicateTargetAnchor {
        target: String,
        packages: Vec<String>,
    },
    DuplicateImplementationIdentity {
        implementation_id: String,
        packages: Vec<String>,
    },
    DuplicateMakeDescriptorBinding {
        binding: String,
        targets: Vec<String>,
    },
    ExtensionHasNoCompatibleAnchor {
        package_id: String,
    },
    OfferOutsidePackageTargets {
        package_id: String,
        implementation_id: String,
    },
    UnknownTarget {
        target: String,
    },
    UnsupportedOutput {
        package_id: String,
        output: SporeOutputKind,
    },
    UnsupportedImplementation {
        target: String,
        base_kind: String,
        implementation_id: String,
    },
}
