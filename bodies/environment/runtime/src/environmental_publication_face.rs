//! Ordinary retained Face projection of kernel-committed generic observations.
//! Face construction uses independently admitted Linux workspace; no new Mask.
use super::environmental_publication::{
    CommittedEnvironmentalObservation, SourceFailure, SourceOutcome,
};
use conduit_environment_face::{contribution, contribution_reading, SensorFailure, SensorStatus};
use conduit_environment_measurement::source_observation::{decode_form, encode_form, FORM_BYTES};
use conduit_face::{Face, FaceBasis, FaceContributionBasis, FaceFragment};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    Basis,
    Sequence,
    Clock,
    Exhausted,
    Face,
}
pub struct RetainedEnvironmentalFace {
    basis: FaceBasis,
    scope: super::environmental_publication::PublicationScope,
    contribution_basis: FaceContributionBasis,
    current: Face,
    reading: Option<[u8; FORM_BYTES]>,
    status: SensorStatus,
    sequence: u64,
    observed_ms: u64,
    last_now: u64,
    fresh: bool,
    stale_after_ms: u64,
}
impl RetainedEnvironmentalFace {
    pub fn prepare(
        basis: FaceBasis,
        scope: &super::environmental_publication::PublicationScope,
        stale_after_ms: u64,
    ) -> Result<Self, Refusal> {
        if stale_after_ms == 0
            || basis.source_document_id.is_none()
            || basis.expanded_plot_id.is_none()
            || basis.body_id.is_some() != basis.wake_id.is_some()
        {
            return Err(Refusal::Basis);
        }
        if basis.source_document_id.as_ref() != Some(&scope.source)
            || basis.checked_plot_id.as_ref() != Some(&scope.checked)
            || basis.expanded_plot_id.as_ref() != Some(&scope.expanded)
            || basis.plan_id.as_ref() != Some(&scope.plan)
            || basis.active_play_id.as_ref() != Some(&scope.play)
        {
            return Err(Refusal::Basis);
        }
        let contribution_basis = FaceContributionBasis {
            checked_plot_id: basis.checked_plot_id.clone().ok_or(Refusal::Basis)?,
            plan_id: basis.plan_id.clone().ok_or(Refusal::Basis)?,
            active_play_id: basis.active_play_id.clone().ok_or(Refusal::Basis)?,
            required_interaction_context: None,
        };
        let fragment = contribution(
            None,
            None,
            false,
            SensorStatus::Warming,
            contribution_basis.clone(),
        )
        .map_err(|_| Refusal::Face)?;
        let current = compose(0, basis.clone(), fragment)?;
        Ok(Self {
            basis,
            scope: scope.clone(),
            contribution_basis,
            current,
            reading: None,
            status: SensorStatus::Warming,
            sequence: 0,
            observed_ms: 0,
            last_now: 0,
            fresh: false,
            stale_after_ms,
        })
    }
    /// Only an opaque snapshot from a committed Back is accepted. Caller owns
    /// the actual selected Source/edge/Host/Boot/Play and observes its own clock;
    /// IDs and this projection never supply missing native authority.
    pub fn publish(
        &mut self,
        snapshot: CommittedEnvironmentalObservation<'_>,
        now: u64,
    ) -> Result<(), Refusal> {
        if snapshot.scope() != &self.scope {
            return Err(Refusal::Basis);
        }
        if self.sequence.checked_add(1) != Some(snapshot.sequence()) {
            return Err(Refusal::Sequence);
        }
        let value = snapshot.value();
        if now < self.last_now
            || value.observed_at_host_boot_ms > now
            || value.observed_at_host_boot_ms < self.observed_ms
        {
            return Err(Refusal::Clock);
        }
        let (reading, status) = match value.outcome {
            SourceOutcome::Available(_) => (
                Some(encode_form(value).map_err(|_| Refusal::Face)?),
                SensorStatus::Available,
            ),
            SourceOutcome::Unavailable(reason) => (
                self.reading,
                SensorStatus::Unavailable(match reason {
                    SourceFailure::CaptureTimeout => SensorFailure::CaptureTimeout,
                    SourceFailure::ProviderFailed => SensorFailure::ProviderFailed,
                    SourceFailure::InvalidSample => SensorFailure::InvalidPayload,
                }),
            ),
        };
        let fresh = matches!(status, SensorStatus::Available)
            && now - value.observed_at_host_boot_ms < self.stale_after_ms;
        let fragment = match reading {
            Some(r) => contribution_reading(
                reading_from_form(&r)?,
                fresh,
                status,
                self.contribution_basis.clone(),
            ),
            None => contribution(None, None, false, status, self.contribution_basis.clone()),
        }
        .map_err(|_| Refusal::Face)?;
        let revision = self
            .current
            .revision
            .checked_add(1)
            .ok_or(Refusal::Exhausted)?;
        let next = compose(revision, self.basis.clone(), fragment)?;
        self.current = next;
        self.reading = reading;
        self.status = status;
        self.sequence = snapshot.sequence();
        self.observed_ms = value.observed_at_host_boot_ms;
        self.last_now = now;
        self.fresh = fresh;
        Ok(())
    }
    pub fn current(&self) -> &Face {
        &self.current
    }
    pub fn at(&mut self, now: u64) -> Result<&Face, Refusal> {
        if now < self.last_now {
            return Err(Refusal::Clock);
        }
        if self.fresh && now - self.observed_ms >= self.stale_after_ms {
            let fragment = contribution_reading(
                reading_from_form(&self.reading.ok_or(Refusal::Face)?)?,
                false,
                self.status,
                self.contribution_basis.clone(),
            )
            .map_err(|_| Refusal::Face)?;
            let next = compose(
                self.current
                    .revision
                    .checked_add(1)
                    .ok_or(Refusal::Exhausted)?,
                self.basis.clone(),
                fragment,
            )?;
            self.current = next;
            self.fresh = false;
        }
        self.last_now = now;
        Ok(&self.current)
    }
}
fn compose(revision: u64, basis: FaceBasis, fragment: FaceFragment) -> Result<Face, Refusal> {
    Face::new_with_semantics_and_temporal(
        revision,
        basis,
        fragment.subjects,
        fragment.relationships,
        fragment.properties,
        fragment.text,
        fragment.actions,
        fragment.disclosures,
        fragment.temporal_references,
        fragment.temporal_facts,
    )
    .map_err(|_| Refusal::Face)
}

fn reading_from_form(
    bytes: &[u8; FORM_BYTES],
) -> Result<conduit_environment_measurement::EnvironmentalReading, Refusal> {
    match decode_form(bytes).map_err(|_| Refusal::Face)?.outcome {
        SourceOutcome::Available(sample) => Ok(sample.reading),
        _ => Err(Refusal::Face),
    }
}
