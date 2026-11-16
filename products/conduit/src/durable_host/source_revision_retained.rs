//! Original selected File/caller custody for already-committed workset recovery.
//! No Source CAS, one-shot deployment action, provider or execution authority.
use super::*;

pub(crate) struct RetainedSourcePreparation {
    source: ImagePreparedValue<OwnedSourceCheckInput>,
    caller: ImagePreparedValue<()>,
    body: conduit_body::BodyId,
    snapshot: SourceSnapshotRevision,
    entries: Vec<String>,
    resident_preparation: Option<ResidentPreparationSelection>,
    adapters: Vec<super::super::local_fore_policy::LocalForeAdapterIntent>,
}
impl PreparedFileSource {
    /// Called only with the same OS selection checked by original preparation.
    /// Keep finite planning intent, never replay admissions/returns/plays.
    pub(in crate::durable_host) fn retain_for_restart(
        self,
        selection: &Selection,
    ) -> Result<(conduit_std_host::StdHost, RetainedSourcePreparation), String> {
        self.host
            .verify_selected_image_prepared_value(&self.source)?;
        self.host
            .verify_selected_image_prepared_value(&self.caller)?;
        if self.source.value().request().snapshot != selection.snapshot {
            return Err("retained planning selection differs from original File capture".into());
        }
        Ok((
            self.host,
            RetainedSourcePreparation {
                source: self.source,
                caller: self.caller,
                body: selection.expected.body_id.clone(),
                snapshot: selection.snapshot.clone(),
                entries: selection.entries.clone(),
                resident_preparation: selection.resident_preparation.clone(),
                adapters: selection.local_fore_adapters.clone(),
            },
        ))
    }
}
impl RetainedSourcePreparation {
    fn verify_capture(&self, host: &conduit_std_host::StdHost) -> Result<(), String> {
        host.verify_selected_image_prepared_value(&self.source)?;
        host.verify_selected_image_prepared_value(&self.caller)?;
        if self.source.value().request().snapshot != self.snapshot {
            return Err("retained planning lost original File snapshot".into());
        }
        Ok(())
    }
    pub(crate) fn restore(
        self,
        host: &conduit_std_host::StdHost,
        biography: &BodyBiographyEvidence,
    ) -> Result<RetainedFileWorkset, String> {
        self.verify_capture(host)?;
        if self.body != biography.body_id {
            return Err("retained planning belongs to another Body".into());
        }
        // Same ordinary checker/entry expansion as adoption, under the SAME
        // retained original caller envelope. Old pre-adoption expected revision
        // is not replayed: the complete checked closure must equal current work.
        let source = crate::plot_source::parse_owned(self.source.value().source().to_owned())?;
        let document = source.check()?;
        let plots = self
            .entries
            .iter()
            .map(|entry| source.expand_checked_for_authoring(&document, entry))
            .collect::<Result<Vec<_>, _>>()?;
        let selected = conduit_body::BodyWorkset::from_plots(plots.iter().map(|plot| {
            conduit_body::ResidentPlot::new(
                plot.expanded.source_document_id.clone(),
                plot.expanded.checked_plot_id.clone(),
            )
        }))
        .map_err(|e| format!("restored File workset: {e:?}"))?;
        if selected != biography.body.workset {
            return Err("restored File closure differs from current committed workset".into());
        }
        let resident_images = self
            .resident_preparation
            .as_ref()
            .map(|policy| policy.prepare(&document, &plots))
            .transpose()?
            .unwrap_or_default();
        drop(document);
        let retained = RetainedFileWorkset {
            checked: CheckedFileRevision {
                source,
                plots,
                resident_images,
            },
            source: self.source,
            caller: self.caller,
            local_fore_adapters: self.adapters,
        };
        retained.verify(host, &biography.body.workset)?;
        Ok(retained)
    }
}
impl RetainedFileWorkset {
    pub(crate) fn requires_local_fore_selection(&self) -> bool {
        !self.local_fore_adapters.is_empty()
    }
    pub(crate) fn select_current_local_fore(
        &self,
        host: &conduit_std_host::StdHost,
        biography: &BodyBiographyEvidence,
        current: &[conduit_core::HostAdvertisement],
    ) -> Result<super::super::local_fore_policy::SelectedLocalForeSet, String> {
        self.verify(host, &biography.body.workset)?;
        super::super::local_fore_policy::SelectedLocalForeSet::prepare(
            host.advertisement(),
            &self.checked.plots,
            current,
            &self.local_fore_adapters,
        )
    }
}
