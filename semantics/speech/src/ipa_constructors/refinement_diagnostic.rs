//! Keep the IPA owner's exact field refusal when Native laws reject first.
use super::{IpaConstructorLocation, SpeechIpaInventoryNotationBasis};
use alloc::{format, string::String};
use conduit_core::StructuredInfoValue;
use conduit_plot::{
    rust_binding::NativeBindingRefusal, NativeRefinementSource, StartupCatalog,
    SyntaxCheckDiagnostic,
};
pub(super) fn install(startup: &mut StartupCatalog) -> Result<(), String> {
    startup.insert_native_refinement_diagnostic(
        "speech/ipa-basis@1",
        SpeechIpaInventoryNotationBasis::semantic_type()
            .map_err(|e| format!("IPA basis Type: {e:?}"))?,
        basis,
    )
}
fn basis(
    value: &StructuredInfoValue,
    source: NativeRefinementSource<'_>,
    _: &NativeBindingRefusal,
) -> Option<SyntaxCheckDiagnostic> {
    // This checks only the basis's internal coherence. Inventory membership
    // remains the ordinary constructor's separate preparation obligation.
    let cause = super::prepare::check_basis_coherence(None, value).err()?;
    let IpaConstructorLocation::Field(path) = cause.location.as_ref() else {
        return None;
    };
    let field = path.strip_prefix("basis.").unwrap_or("");
    Some(SyntaxCheckDiagnostic {
        code: "CND-SPC-IPA",
        span: source.field_span(field)?,
        message: format!("{:?}", cause.refusal),
    })
}
