//! Only actual completed process outputs enter this preparation path.
use super::*;
use crate::source_check::{requests, PreparedSourceChecker, SourceCheckPreparationBounds};
impl PreparedSourceChecker {
    /// Reuses the existing immutable provider. The caller independently owns
    /// Source grants, selected artifact and Root metadata/admission workspace.
    /// The supplied schema is an already prepared checked declaration; nothing
    /// here restores a compiler owner from a copied report or creates a grant.
    pub fn from_process_outputs(
        schema: &SourceCheckNativeSchema,
        outputs: &[SourceCheckProcessOutput],
        bounds: SourceCheckPreparationBounds,
        control: &crate::RunControl,
    ) -> Result<Self, String> {
        let checkpoint = || {
            if control.stop_requested() {
                Err("Source-check preparation cancelled".into())
            } else {
                Ok(())
            }
        };
        checkpoint()?;
        Self::validate_bounds(outputs.len(), bounds)?;
        let mut builder = requests::Builder::new(outputs.len());
        for output in outputs {
            checkpoint()?;
            let request = schema
                .encode_request(output.request(), bounds.request_bytes as usize)
                .map_err(|error| format!("process request: {error:?}"))?;
            schema
                .admit_reply(output.canonical_bytes(), bounds.report_bytes as usize)
                .map_err(|error| format!("process reply: {error:?}"))?;
            correlate(&request, output.canonical_bytes())?;
            builder.push(&request, output.canonical_bytes())?;
        }
        Self::assemble(schema, builder, bounds, checkpoint)
    }
}

fn correlate(request: &[u8], reply: &[u8]) -> Result<(), String> {
    use conduit_core::validate_canonical_structured_value as validate;
    let request = validate(request).map_err(|e| format!("request shape: {e:?}"))?;
    let reply = validate(reply).map_err(|e| format!("reply shape: {e:?}"))?;
    // A refused preparation cannot establish a current Source/context scope.
    // Runtime stale/context refusals remain the existing provider's responsibility.
    let report = reply
        .variant_payload("checked")
        .map_err(|e| format!("reply variant: {e:?}"))?
        .ok_or("process did not establish a prepared Source context")?;
    for (given, wanted) in [
        ("snapshot", "snapshot"),
        ("context", "context"),
        ("requested_entry", "entry"),
    ] {
        let given = report
            .record_field(given)
            .map_err(|e| format!("report field: {e:?}"))?
            .ok_or("report field absent")?;
        let wanted = request
            .record_field(wanted)
            .map_err(|e| format!("request field: {e:?}"))?
            .ok_or("request field absent")?;
        if given.value_node() != wanted.value_node() {
            return Err("process report differs from its exact request".into());
        }
    }
    Ok(())
}
