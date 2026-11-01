//! Compile fixed pristine evidence instead of retaining a general history
//! deserializer on constrained Hosts. The original biography stays in custody.
use super::*;
use std::fmt::{self, Write};
impl PreparedBodyImage {
    /// Reconstruct the exact generated representation on the construction host.
    /// Equality with the retained biography is mandatory, not just matching IDs:
    /// a future non-pristine/emergency image must not silently lose its fields.
    pub fn reconstruct_bootstrap(&self) -> Result<BodyBiographyEvidence, &'static str> {
        let record = self
            .evidence
            .records
            .first()
            .ok_or("body-bootstrap-record")?;
        let body = conduit_body::Body::born_with_plots(
            self.evidence.body.workset.clone(),
            record.sequence,
            record.sign_id.clone(),
        )
        .map_err(|_| "body-bootstrap-reconstruction")?;
        let membership = conduit_body::BodyMembership::new(body.body_id.clone())
            .map_err(|_| "body-bootstrap-membership")?;
        let restored =
            BodyBiographyEvidence::born(body, membership, self.evidence.friendly_name.clone())
                .map_err(|_| "body-bootstrap-biography")?;
        if restored != self.evidence {
            return Err("body-bootstrap-not-pristine");
        }
        Ok(restored)
    }
    pub fn write_bootstrap_rust(&self, out: &mut impl Write) -> fmt::Result {
        self.reconstruct_bootstrap().map_err(|_| fmt::Error)?;
        let record = &self.evidence.records[0];
        writeln!(
            out,
            "// Same Make-born pristine biography; no runtime Crèche, new Sign or grant."
        )?;
        writeln!(
            out,
            "pub fn evidence() -> Result<conduit_body::BodyBiographyEvidence, &'static str> {{"
        )?;
        writeln!(out, "let workset=conduit_body::BodyWorkset::from_plots([")?;
        for plot in self.evidence.body.workset.plots() {
            writeln!(
                out,
                "conduit_body::ResidentPlot::new({:?}.into(),{:?}.into()),",
                plot.source_document_id.as_str(),
                plot.checked_plot_id.as_str()
            )?;
        }
        writeln!(out, "]).map_err(|_|\"body-image-workset\")?;")?;
        writeln!(out,"conduit_environment_runtime::body_image::restore_pristine_image(workset,{}, {:?}.into(), {:?}.into(), {:?}).map_err(|_|\"body-image-reconstruction\") }}",
            record.sequence,record.sign_id.as_str(),self.evidence.friendly_name,self.evidence.body_id.as_str())
    }
}
