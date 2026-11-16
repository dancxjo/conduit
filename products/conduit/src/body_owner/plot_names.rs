//! Checked workset labels, independently funded on the original Root counter.
//! Labels describe exact resident identities; they confer no execution authority.
use super::Owner;
use conduit_body::{BodyWorkset, ResidentPlot};
use conduit_face::FaceResidentPlotName;
use conduit_std_host::body_execution::{ImagePreparedValue, ImageRootPreparationOwner};
use std::mem::size_of;

type Entry = (ResidentPlot, String);
type Label<'a> = (
    &'a conduit_core::SourceDocumentId,
    &'a conduit_core::CheckedPlotId,
    &'a str,
);
#[derive(Default)]
pub(super) enum PlotNames {
    #[default]
    Empty,
    Image(ImagePreparedValue<Vec<Entry>>),
    Hosted(Vec<Entry>),
}
impl PlotNames {
    fn entries(&self) -> &[Entry] {
        match self {
            Self::Empty => &[],
            Self::Image(value) => value.value(),
            Self::Hosted(value) => value,
        }
    }
    /// All inputs are borrowed checked facts, already owned by the caller.
    /// Reserve the replacement before allocating; the old table stays funded
    /// until successful publication. No mutation/growth follows construction.
    pub(super) fn prepare<'a>(
        workset: &BodyWorkset,
        labels: impl Iterator<Item = Label<'a>> + Clone,
        owner: Option<&ImageRootPreparationOwner>,
    ) -> Result<Self, String> {
        let mut count = 0usize;
        let mut bytes = size_of::<Vec<Entry>>();
        for (source, checked, name) in labels.clone() {
            count += 1;
            if count > conduit_body::MAX_BODY_PLOTS
                || !workset
                    .plots()
                    .iter()
                    .any(|p| &p.source_document_id == source && &p.checked_plot_id == checked)
                || name.is_empty()
                || name.len() > conduit_plot::MAX_CHECK_LABEL_BYTES
                || labels
                    .clone()
                    .filter(|(s, c, _)| *s == source && *c == checked)
                    .count()
                    != 1
            {
                return Err("Plot labels differ from exact bounded checked workset".into());
            }
            bytes = bytes
                .checked_add(size_of::<Entry>())
                .and_then(|n| n.checked_add(source.as_str().len()))
                .and_then(|n| n.checked_add(checked.as_str().len()))
                .and_then(|n| n.checked_add(name.len()))
                .ok_or("Plot label backing overflow")?;
        }
        let build = || {
            let mut entries = Vec::new();
            entries
                .try_reserve_exact(count)
                .map_err(|_| "Plot label allocation refused")?;
            for (source, checked, name) in labels {
                entries.push((
                    ResidentPlot::new(source.clone(), checked.clone()),
                    name.to_owned(),
                ));
            }
            let actual = size_of::<Vec<Entry>>()
                + entries.capacity() * size_of::<Entry>()
                + entries
                    .iter()
                    .map(|(p, n)| {
                        p.source_document_id.0.capacity()
                            + p.checked_plot_id.0.capacity()
                            + n.capacity()
                    })
                    .sum::<usize>();
            if actual > bytes {
                return Err("Plot label backing exceeds admitted envelope".into());
            }
            Ok(entries)
        };
        match owner {
            Some(owner) => {
                // SAFETY: one exactly reserved Vec, at most MAX_BODY_PLOTS
                // entries, three exact String copies per entry; no external
                // effects or retained captures. The immutable result owns every
                // copied byte until its original IMAGE charge drops.
                unsafe { owner.prepare_value(bytes, bytes, build) }.map(Self::Image)
            }
            None => build().map(Self::Hosted),
        }
    }
}
impl Owner {
    pub(super) fn observe_plot_name(
        &mut self,
        checked: &conduit_plot::ExpandedAuthoringPlot,
    ) -> Result<(), String> {
        let source = &checked.expanded.source_document_id;
        let checked_id = &checked.expanded.checked_plot_id;
        let workset = &self.session.evidence().body.workset;
        let labels = self
            .plot_names
            .entries()
            .iter()
            .filter(|(p, _)| {
                workset.contains(p)
                    && (&p.source_document_id != source || &p.checked_plot_id != checked_id)
            })
            .map(|(p, n)| (&p.source_document_id, &p.checked_plot_id, n.as_str()))
            .chain(std::iter::once((
                source,
                checked_id,
                checked.expanded.name.as_str(),
            )));
        let names = PlotNames::prepare(workset, labels, self.host.optional_root_preparation())?;
        self.plot_names = names;
        Ok(())
    }
    pub(super) fn face_plot_names(&self) -> Vec<FaceResidentPlotName<'_>> {
        let mut names: Vec<_> = self
            .plot_names
            .entries()
            .iter()
            .filter(|(plot, _)| self.session.evidence().body.workset.contains(plot))
            .map(|(plot, name)| FaceResidentPlotName {
                source_document_id: &plot.source_document_id,
                checked_plot_id: &plot.checked_plot_id,
                name,
            })
            .collect();
        // Single-Plot transitions also have a checked foreground label. Prefer
        // that current label over an older table entry for the same identity.
        if let Some((plot, name)) = self.resident.as_ref().zip(self.resident_name.as_deref()) {
            names.retain(|entry| {
                entry.source_document_id != &plot.source_document_id
                    || entry.checked_plot_id != &plot.checked_plot_id
            });
            names.push(FaceResidentPlotName {
                source_document_id: &plot.source_document_id,
                checked_plot_id: &plot.checked_plot_id,
                name,
            });
        }
        names
    }
}
#[cfg(test)]
#[path = "plot_names/tests.rs"]
mod tests;
