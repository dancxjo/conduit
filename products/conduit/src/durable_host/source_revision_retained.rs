//! Original selected File/caller custody for already-committed workset recovery.
//! No Source CAS, one-shot deployment action, provider or execution authority.
use super::*;

pub(crate) struct RetainedSourcePreparation {
    body: conduit_body::BodyId,
    snapshot: SourceSnapshotRevision,
    entries: Vec<String>,
    resident_preparation: Option<ResidentPreparationSelection>,
    #[cfg_attr(
        not(all(feature = "root-allocation-observation", not(test))),
        allow(dead_code)
    )]
    caller_workspace_bytes: usize,
    adapters: Vec<super::super::local_fore_policy::LocalForeAdapterIntent>,
    original_policy_fingerprint: [u8; 32],
    // Drop derived planning metadata before releasing its original IMAGE guards.
    source: ImagePreparedValue<OwnedSourceCheckInput>,
    caller: ImagePreparedValue<()>,
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
        if self.host.advertisement().host_id.as_str() != selection.host_id
            || self.caller_workspace_bytes != selection.caller_workspace_bytes
            || self.source.value().request().snapshot != selection.snapshot
        {
            return Err("retained planning selection differs from original File capture".into());
        }
        Ok((
            self.host,
            RetainedSourcePreparation {
                original_policy_fingerprint: selection.execution_policy_fingerprint()?,
                source: self.source,
                caller: self.caller,
                body: selection.expected.body_id.clone(),
                snapshot: selection.snapshot.clone(),
                entries: selection.entries.clone(),
                resident_preparation: selection.resident_preparation.clone(),
                caller_workspace_bytes: selection.caller_workspace_bytes,
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
    fn check_current(
        &self,
        host: &conduit_std_host::StdHost,
        biography: &BodyBiographyEvidence,
    ) -> Result<CheckedFileRevision, String> {
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
        #[cfg(all(feature = "root-allocation-observation", not(test)))]
        if conduit_std_host::allocation_observation::snapshot().peak_requested_bytes
            > self.caller_workspace_bytes
        {
            return Err("restored Source checking exceeds original caller envelope".into());
        }
        Ok(CheckedFileRevision {
            source,
            plots,
            resident_images,
        })
    }
    pub(crate) fn restore(
        self,
        host: &conduit_std_host::StdHost,
        biography: &BodyBiographyEvidence,
    ) -> Result<RetainedFileWorkset, String> {
        let checked = self.check_current(host, biography)?;
        let retained = RetainedFileWorkset {
            original_policy_fingerprint: self.original_policy_fingerprint,
            body_id: self.body,
            checked,
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
    pub(crate) fn verify_planning(
        &self,
        host: &conduit_std_host::StdHost,
        biography: &BodyBiographyEvidence,
    ) -> Result<(), String> {
        if self.body_id != biography.body_id {
            return Err("retained Fore planning belongs to another Body".into());
        }
        self.verify(host, &biography.body.workset)
    }
    pub(crate) fn select_current_local_fore(
        &self,
        host: &conduit_std_host::StdHost,
        biography: &BodyBiographyEvidence,
        current: &[conduit_core::HostAdvertisement],
    ) -> Result<super::super::local_fore_policy::SelectedLocalForeSet, String> {
        self.verify_planning(host, biography)?;
        super::super::local_fore_policy::SelectedLocalForeSet::prepare(
            host.advertisement(),
            &self.checked.plots,
            current,
            &self.local_fore_adapters,
        )
    }
}

#[cfg(test)]
#[path = "source_revision_retained_tests.rs"]
pub(super) mod tests;
