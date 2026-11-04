//! Normalize admitted Native proposal Info; admission is shape/law custody only.
use super::*;
use conduit_core::{StructuredInfoValue as Value, StructuredInfoValueShape as Shape};
use conduit_plot::{rust_binding::AdmittedNativeCanonical, CheckedSyntaxDocument};

/// The caller pins its actual checked declaration owner independently of input.
/// Matching public IDs do not replace that owner. This converts conditional Info
/// only: no grant, checked compiler value, lifecycle effect or adoption results.
pub fn propose_admitted_workset(
    observed: &Body,
    declaration: &CheckedSyntaxDocument,
    admitted: &AdmittedNativeCanonical<'_>,
    candidate: Option<CheckedWorksetCandidate<'_>>,
) -> Result<BodyWorksetProposalInfoV1, WorksetProposalRefusal> {
    use WorksetProposalRefusal as E;
    let native = declaration
        .native_types
        .iter()
        .find(|native| native.name == "WorksetProposalInfo")
        .ok_or(E::NativeSchema)?;
    // Public inspection metadata is not the retained Native owner. Reuse the
    // existing private-catalog correlation instead of treating it as a witness.
    let semantic = native
        .value_type
        .canonical_bytes()
        .map_err(|_| E::NativeSchema)?;
    let owner = conduit_plot::rust_binding::check_prepared_decode_native(
        declaration,
        native.identity.as_str(),
        &semantic,
        &native.value_contracts,
        &native.invariants,
    )
    .map_err(|_| E::NativeSchema)?;
    if owner.name != "WorksetProposalInfo" {
        return Err(E::NativeSchema);
    }
    if !core::ptr::eq(declaration, admitted.source_document())
        || !core::ptr::eq(owner, admitted.native_type())
    {
        return Err(E::NativeOwner);
    }
    if admitted.canonical_bytes().len() > MAX_WORKSET_PROPOSAL_BYTES {
        return Err(E::Bytes);
    }
    let value =
        Value::from_canonical_bytes(admitted.canonical_bytes()).map_err(|_| E::NativeSchema)?;
    if bytes(field(&value, "schema_version")?)? != 1u16.to_le_bytes() {
        return Err(E::NativeSchema);
    }
    if text(field(&value, "expected_body_id")?)? != observed.body_id.as_str() {
        return Err(E::WrongBody);
    }
    let revision = u64_value(field(&value, "expected_workload_revision")?)?;
    let Shape::Collection(values) = field(&value, "proposed_workset")?.shape() else {
        return Err(E::NativeSchema);
    };
    if values.len() > conduit_body::MAX_BODY_PLOTS {
        return Err(E::NativeSchema);
    }
    let plots = values
        .iter()
        .map(|plot| {
            Ok(ResidentPlot::new(
                text(field(plot, "source_document_id")?)?.into(),
                text(field(plot, "checked_plot_id")?)?.into(),
            ))
        })
        .collect::<Result<Vec<_>, E>>()?;
    let Shape::Variant { tag, payload } = field(&value, "candidate_source")?.shape() else {
        return Err(E::NativeSchema);
    };
    match (tag, candidate.as_ref()) {
        ("none", None) => {}
        ("checked", Some(candidate)) => check_source_basis(payload, candidate.report)?,
        _ => return Err(E::NativeSourceBasis),
    }
    propose_checked_workset(observed, &observed.body_id, revision, plots, candidate)
}
fn check_source_basis(
    value: &Value,
    report: &SourceCheckInfoV1,
) -> Result<(), WorksetProposalRefusal> {
    use WorksetProposalRefusal as E;
    let snapshot = field(value, "snapshot")?;
    if text(field(snapshot, "owner")?)? != report.snapshot.owner
        || u64_value(field(snapshot, "generation")?)? != report.snapshot.generation
        || text(field(snapshot, "sha256")?)? != report.snapshot.sha256
        || text(field(snapshot, "source_document_id")?)? != report.snapshot.source_document_id
    {
        return Err(E::NativeSourceBasis);
    }
    let context = field(value, "context")?;
    for (name, expected) in [
        ("grammar_edition", &report.context.grammar_edition),
        ("catalog", &report.context.catalog),
        ("kinds_fores", &report.context.kinds_fores),
        ("types_laws", &report.context.types_laws),
        ("backs", &report.context.backs),
    ] {
        if text(field(context, name)?)? != expected {
            return Err(E::NativeSourceBasis);
        }
    }
    if text(field(value, "entry")?)? != report.requested_entry
        || Some(text(field(value, "checked_plot_id")?)?)
            != report.candidate.checked_plot_id.as_deref()
    {
        return Err(E::NativeSourceBasis);
    }
    Ok(())
}
fn field<'a>(value: &'a Value, name: &str) -> Result<&'a Value, WorksetProposalRefusal> {
    let Shape::Record(fields) = value.shape() else {
        return Err(WorksetProposalRefusal::NativeSchema);
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .map(|field| field.value())
        .ok_or(WorksetProposalRefusal::NativeSchema)
}
fn bytes(value: &Value) -> Result<&[u8], WorksetProposalRefusal> {
    match value.shape() {
        Shape::Leaf(bytes) => Ok(bytes),
        _ => Err(WorksetProposalRefusal::NativeSchema),
    }
}
fn text(value: &Value) -> Result<&str, WorksetProposalRefusal> {
    core::str::from_utf8(bytes(value)?).map_err(|_| WorksetProposalRefusal::NativeSchema)
}
fn u64_value(value: &Value) -> Result<u64, WorksetProposalRefusal> {
    Ok(u64::from_le_bytes(
        bytes(value)?
            .try_into()
            .map_err(|_| WorksetProposalRefusal::NativeSchema)?,
    ))
}
