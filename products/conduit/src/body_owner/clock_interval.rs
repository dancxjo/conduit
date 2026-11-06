//! Checked Face controls for the installed owner's interval ticker.
//! An interval change replaces the Body workset only while lulled; a start
//! seals a new Plan and Play, and a stop requests a real terminal receipt.
use super::{debug, Owner};
use conduit_body::{BodyState, ResidentPlot};
use conduit_core::{kind_id, CheckedValueContract, ValueConstraint};
use conduit_face::{
    Face, FaceAction, FaceActionArgument, FaceActionAvailability, FaceDisclosureLevel,
    FaceInteraction, FaceProperty, FacePropertyValue, FaceText, MaskShow, UTF8_TEXT_VALUE_KIND,
};
use std::path::Path;

/// Affine custody minted only by the clock's finite Source policy below.
/// No public proposal, serialized identity or arbitrary Source can construct it.
pub(super) struct AuthorizedClockRevision {
    expected: conduit_body::BodyBiographyEvidence,
    owner: (
        conduit_core::HostId,
        conduit_core::BootId,
        conduit_core::OfferGeneration,
    ),
    source: crate::plot_source::CanonicalSource,
    checked: conduit_plot::ExpandedAuthoringPlot,
}
impl AuthorizedClockRevision {
    pub(super) fn into_parts(
        self,
    ) -> (
        conduit_body::BodyBiographyEvidence,
        (
            conduit_core::HostId,
            conduit_core::BootId,
            conduit_core::OfferGeneration,
        ),
        crate::plot_source::CanonicalSource,
        conduit_plot::ExpandedAuthoringPlot,
    ) {
        (self.expected, self.owner, self.source, self.checked)
    }
}

const INITIAL_SOURCE: &str = include_str!("../../../../plots/clock/main.conduit");
const INTERVALS_MS: [u64; 4] = [250, 500, 1_000, 2_000];
pub(super) const CLOCK_INTERVAL_ACTION: &str = "conduit.intent/change-clock-interval@1";
pub(super) const CLOCK_START_ACTION: &str = "conduit.intent/start-clock@1";
pub(super) const CLOCK_LULL_ACTION: &str = "conduit.intent/lull-clock@1";
const BODY_WAKE_ACTION: &str = "conduit.intent/wake@1";
const BODY_LULL_ACTION: &str = "conduit.intent/lull@1";
pub(crate) const CLOCK_RUN_MAXIMUM_MILLIS: u64 = 60_000;

pub(crate) fn is_clock_control_intent(intent: &str) -> bool {
    intent == CLOCK_INTERVAL_ACTION || intent == CLOCK_START_ACTION || intent == CLOCK_LULL_ACTION
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ClockAction {
    ChangeInterval,
    Start,
    Lull,
    NotebookNavigation,
}

pub(super) fn source_for(interval_ms: u64) -> String {
    format!(
        "plot clock-demo {{\n    # Standing clock: bounded instantaneous pressure, alive until Stop.\n    clock: time/every({interval_ms}ms)\n    clock >> face/tick(\"Clock tick\")\n}}\n"
    )
}

#[cfg(test)]
fn supported_interval(source: &str) -> Option<u64> {
    if source == INITIAL_SOURCE {
        return Some(1_000);
    }
    INTERVALS_MS
        .into_iter()
        .find(|interval| source == source_for(*interval))
}

pub(super) fn recognized_interval(resident: &ResidentPlot) -> Option<u64> {
    std::iter::once((1_000, INITIAL_SOURCE.to_owned()))
        .chain(INTERVALS_MS.into_iter().map(|ms| (ms, source_for(ms))))
        .find_map(|(ms, source)| {
            crate::plot_source::parse(&source)
                .and_then(|plot| plot.expand_entry_for_authoring())
                .ok()
                .filter(|checked| {
                    ResidentPlot::new(
                        checked.expanded.source_document_id.clone(),
                        checked.expanded.checked_plot_id.clone(),
                    ) == *resident
                })
                .map(|_| ms)
        })
}

pub(super) fn with_clock_action(owner: &Owner, face: Face) -> Result<Face, String> {
    let Some(interval_ms) = owner.clock_interval_ms else {
        return Ok(face);
    };
    let resident = owner
        .resident
        .as_ref()
        .ok_or("clock has no resident Plot")?;
    let target = format!("plot/{}", resident.checked_plot_id.as_str());
    let mut disclosures = face.disclosures.clone();
    disclosures
        .iter_mut()
        .find(|disclosure| disclosure.subject == target)
        .ok_or("clock Plot has no Face disclosure")?
        .level = FaceDisclosureLevel::Primary;
    let lulled = owner.session.evidence().body.state == BodyState::Lulled
        && owner.session.realization().is_none()
        && !owner.host.is_playing();
    #[cfg(unix)]
    let terminal_attached = lulled && owner.host.current().terminal_attachment_is_live()?;
    #[cfg(not(unix))]
    let terminal_attached = false;
    let mut members: Vec<_> = INTERVALS_MS
        .into_iter()
        .map(|interval| interval.to_string().into_bytes())
        .collect();
    members.sort();
    let argument = FaceActionArgument {
        name: "clock/interval-ms".into(),
        value_name: "Time between ticks in milliseconds: 250, 500, 1000, or 2000".into(),
        contract: CheckedValueContract::new(
            kind_id(UTF8_TEXT_VALUE_KIND),
            4,
            vec![ValueConstraint::CanonicalMembership {
                members,
                negated: false,
            }],
        )
        .map_err(debug)?,
    };
    let mut subjects = face.subjects.clone();
    subjects
        .iter_mut()
        .find(|subject| subject.identity == target)
        .ok_or("ticker Plot has no Face subject")?
        .name = "Interval ticker".into();
    let mut properties = face.properties.clone();
    properties.push(FaceProperty {
        subject: target.clone(),
        name: "interval-ms".into(),
        value: FacePropertyValue::Count(interval_ms),
    });
    let mut text = face.text.clone();
    text.push(FaceText {
        subject: target.clone(),
        text: if owner.current_play_id().is_some() && owner.host.is_playing() {
            "The ticker is running."
        } else if lulled {
            "The ticker is stopped."
        } else {
            "The ticker is waiting to start."
        }
        .into(),
    });
    text.push(FaceText {
        subject: target.clone(),
        text: format!("The ticker emits a pulse every {interval_ms} milliseconds."),
    });
    let mut actions = face.actions.clone();
    for action in &mut actions {
        if action.intent == BODY_WAKE_ACTION && terminal_attached {
            action.availability = FaceActionAvailability::Unavailable {
                reason_code: "terminal-mask-attached".into(),
                explanation: "Detach the terminal Mask before waking this ticker Body.".into(),
            };
        } else if action.intent == BODY_LULL_ACTION && owner.current_play_id().is_none() {
            action.availability = FaceActionAvailability::Unavailable {
                reason_code: "clock-not-playing".into(),
                explanation: "The ticker Play has not started yet.".into(),
            };
        } else if !matches!(action.intent.as_str(), BODY_WAKE_ACTION | BODY_LULL_ACTION)
            && action.availability.is_available()
        {
            action.availability = FaceActionAvailability::Unavailable {
                reason_code: "owner-action-return-not-admitted".into(),
                explanation: "This owner return route does not yet accept this semantic action."
                    .into(),
            };
        }
    }
    actions.push(FaceAction {
        identity: format!(
            "body/action/change-clock-interval/{}",
            owner.session.evidence().body.workload_revision
        ),
        intent: CLOCK_INTERVAL_ACTION.into(),
        target: target.clone(),
        name: "Change ticker pace".into(),
        arguments: vec![argument],
        disclosure: FaceDisclosureLevel::CurrentAction,
        availability: if lulled {
            FaceActionAvailability::Available
        } else {
            FaceActionAvailability::Unavailable {
                reason_code: "clock-play-must-lull".into(),
                explanation: "Lull the current ticker Play before changing its checked interval. The next start will require a replacement Plan.".into(),
            }
        },
    });
    actions.push(FaceAction {
        identity: format!(
            "body/action/start-clock/{}",
            owner.session.evidence().body.workload_revision
        ),
        intent: CLOCK_START_ACTION.into(),
        target: target.clone(),
        name: "Start the ticker".into(),
        arguments: vec![],
        disclosure: FaceDisclosureLevel::CurrentAction,
        availability: if lulled && !terminal_attached {
            FaceActionAvailability::Available
        } else if terminal_attached {
            FaceActionAvailability::Unavailable {
                reason_code: "terminal-mask-attached".into(),
                explanation: "Detach the terminal Mask before starting a new ticker Play.".into(),
            }
        } else {
            FaceActionAvailability::Unavailable {
                reason_code: "clock-already-started".into(),
                explanation: "The ticker needs to finish or be lulled before it starts again."
                    .into(),
            }
        },
    });
    actions.push(FaceAction {
        identity: format!(
            "body/action/lull-clock/{}",
            owner.current_play_id().map_or("none", |play| play.as_str())
        ),
        intent: CLOCK_LULL_ACTION.into(),
        target,
        name: "Stop the ticker".into(),
        arguments: vec![],
        disclosure: FaceDisclosureLevel::CurrentAction,
        availability: if owner.current_play_id().is_some() {
            FaceActionAvailability::Available
        } else {
            FaceActionAvailability::Unavailable {
                reason_code: "clock-not-playing".into(),
                explanation: "Start the ticker before stopping it.".into(),
            }
        },
    });
    let revised = Face::new_with_semantics_and_temporal(
        face.revision,
        face.basis,
        subjects,
        face.relationships,
        properties,
        text,
        actions,
        disclosures,
        face.temporal_references,
        face.temporal_facts,
    )
    .and_then(|value| value.with_composition(face.composition))
    .and_then(|value| value.with_interaction_context(face.interaction_context))
    .map_err(debug)?;
    revised.validate().map_err(debug)?;
    Ok(revised)
}

impl Owner {
    pub(crate) fn resolve_clock_interaction(
        &self,
        show: &MaskShow,
        interaction: &FaceInteraction,
    ) -> Result<ClockAction, String> {
        let face = self.local_face_snapshot()?;
        show.validate(&face)
            .map_err(|error| format!("stale owner Mask Show: {error:?}"))?;
        interaction
            .validate_against(&face, show)
            .map_err(|error| format!("owner Face interaction refused: {error:?}"))?;
        let action = face
            .resolve_action(interaction.face_revision, &interaction.action_id)
            .map_err(debug)?;
        // The installed clock's Wake prepares its current checked workset and
        // starts the owner Play; Lull requests that same Play's terminal sign.
        // These are the lifecycle operations performed by Start and Stop below,
        // through one owner path rather than a Mask-owned state transition.
        match (action.intent.as_str(), interaction.arguments.len()) {
            (CLOCK_INTERVAL_ACTION, 1) => Ok(ClockAction::ChangeInterval),
            (CLOCK_START_ACTION | BODY_WAKE_ACTION, 0) => Ok(ClockAction::Start),
            (CLOCK_LULL_ACTION | BODY_LULL_ACTION, 0) => Ok(ClockAction::Lull),
            (
                "conduit.intent/open-body-overview@1"
                | "conduit.intent/open-plot-library@1"
                | "conduit.intent/open-resident-plot@1",
                0,
            ) if self.has_open_notebook() && self.current_play_id().is_some() => {
                Ok(ClockAction::NotebookNavigation)
            }
            _ => Err("owner action is not a current clock control".into()),
        }
    }

    /// The local control token authenticates the caller; the same current
    /// Face and acknowledged Mask Show constrain the semantic action itself.
    pub(crate) fn apply_clock_interval_interaction(
        &mut self,
        root: &Path,
        show: &MaskShow,
        interaction: &FaceInteraction,
    ) -> Result<serde_json::Value, String> {
        if self.resolve_clock_interaction(show, interaction)? != ClockAction::ChangeInterval {
            return Err("owner action is not a clock interval change".into());
        }
        self.apply_admitted_clock_interval(root, interaction)
    }

    /// The ordinary return or Commons adapter must first validate its own
    /// current Face, typed arguments and actual acknowledged Show. This method
    /// retains the single checked-source/workset effect path.
    pub(super) fn apply_admitted_clock_interval(
        &mut self,
        root: &Path,
        interaction: &FaceInteraction,
    ) -> Result<serde_json::Value, String> {
        let interval_text = std::str::from_utf8(&interaction.arguments[0].value)
            .map_err(|_| "clock interval is not UTF-8".to_string())?;
        let interval_ms = interval_text
            .parse::<u64>()
            .map_err(|_| "clock interval is not a decimal millisecond count".to_string())?;
        self.replace_authorized_clock_interval(root, interval_ms)?;
        Ok(serde_json::json!({
            "schema":"conduit.body/clock-interval-changed@1",
            "body_id":self.session.evidence().body_id,
            "workload_revision":self.session.evidence().body.workload_revision,
            "interval_ms":interval_ms,
            "prior_face_id":interaction.face_id,
            "prior_face_revision":interaction.face_revision,
            "prior_show_id":interaction.show_id,
            "interaction_id":interaction.identity.as_str(),
            "next_step":"start-replacement-plan",
            "play_state":"lulled"
        }))
    }

    /// Replace the checked resident clock with another admitted interval.
    /// This is a workset change, not a timer-setting side channel. Returning
    /// success leaves the Body lulled: a subsequent start creates a new Plan
    /// and Play from the retained new checked source.
    #[cfg(test)]
    pub(crate) fn replace_lulled_clock_interval(
        &mut self,
        root: &Path,
        interval_ms: u64,
    ) -> Result<(), String> {
        // Trusted reference fixture; production enters through admitted Face actions.
        self.replace_authorized_clock_interval(root, interval_ms)
    }

    fn replace_authorized_clock_interval(
        &mut self,
        root: &Path,
        interval_ms: u64,
    ) -> Result<(), String> {
        if !INTERVALS_MS.contains(&interval_ms) {
            return Err("clock interval must be 250, 500, 1000, or 2000 milliseconds".into());
        }
        if self.host.is_playing()
            || self.session.realization().is_some()
            || self.session.evidence().body.state != BodyState::Lulled
        {
            return Err("clock interval change requires a lulled Body and a retired Play".into());
        }
        if self.session.evidence().body.workset.len() != 1 {
            return Err("reviewed clock interval change requires the single-clock workset".into());
        }
        self.ensure_execution_retained(root)?;
        let old_checked = super::super::checked_retained_source(root)?
            .ok_or("retained clock Source is unavailable")?;
        let old_resident = ResidentPlot::new(
            old_checked.expanded.source_document_id,
            old_checked.expanded.checked_plot_id,
        );
        let old_interval = recognized_interval(&old_resident)
            .ok_or("retained Plot is not the reviewed clock source")?;
        if old_interval == interval_ms {
            return Err("clock already has that interval".into());
        }
        if self.resident.as_ref() != Some(&old_resident) {
            return Err("retained clock source differs from the Body workset".into());
        }
        let next_source = source_for(interval_ms);
        let canonical = crate::plot_source::parse(&next_source)?;
        let next_checked = canonical.expand_entry_for_authoring()?;
        let expected = self.session.evidence().clone();
        let advertised = self.host.advertisement();
        let authorization = AuthorizedClockRevision {
            expected,
            owner: (
                advertised.host_id.clone(),
                advertised.boot_id.clone(),
                advertised.offer_generation,
            ),
            source: canonical,
            checked: next_checked,
        };
        self.commit_authorized_clock_revision(root, authorization)?;
        self.clock_interval_ms = Some(interval_ms);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_reviewed_clock_sources_are_eligible() {
        assert_eq!(supported_interval(INITIAL_SOURCE), Some(1_000));
        for interval in INTERVALS_MS {
            assert_eq!(supported_interval(&source_for(interval)), Some(interval));
        }
        assert_eq!(supported_interval("plot clock-demo {}"), None);
        assert_eq!(supported_interval(&source_for(3_000)), None);
    }
    #[test]
    fn private_approval_refuses_foreign_host_boot_and_offer_generation() {
        use super::super::service_workset_tests::fixture;
        let (root, mut owner) = fixture();
        let expected = owner.session.evidence().clone();
        let before = std::fs::read(root.join("body/biography.json")).unwrap();
        for changed in 0..3 {
            let source = crate::plot_source::parse(&source_for(500)).unwrap();
            let checked = source.expand_entry_for_authoring().unwrap();
            let ad = owner.host.advertisement();
            let mut origin = (ad.host_id.clone(), ad.boot_id.clone(), ad.offer_generation);
            match changed {
                0 => origin.0 = "foreign/host".into(),
                1 => origin.1 = "foreign/boot".into(),
                _ => origin.2 = conduit_core::OfferGeneration(ad.offer_generation.0 + 1),
            }
            // Deliberately malformed private token: this module alone can
            // construct it. Public proposal Info cannot take this entrance.
            let approval = AuthorizedClockRevision {
                expected: expected.clone(),
                owner: origin,
                source,
                checked,
            };
            assert!(owner
                .commit_authorized_clock_revision(&root, approval)
                .unwrap_err()
                .contains("another Host/Boot or offer generation"));
            assert_eq!(owner.session.evidence(), &expected);
            assert_eq!(
                std::fs::read(root.join("body/biography.json")).unwrap(),
                before
            );
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn consumed_approval_cannot_overwrite_a_later_workset_basis() {
        use super::super::service_workset_tests::{fixture, SOURCE};
        let (root, mut owner) = fixture();
        let expected = owner.session.evidence().clone();
        let source = crate::plot_source::parse(&source_for(500)).unwrap();
        let checked = source.expand_entry_for_authoring().unwrap();
        let ad = owner.host.advertisement();
        let approval = AuthorizedClockRevision {
            expected: expected.clone(),
            owner: (ad.host_id.clone(), ad.boot_id.clone(), ad.offer_generation),
            source,
            checked,
        };
        let retained = crate::plot_source::parse(SOURCE).unwrap();
        let alpha = retained.expand_named_for_authoring("alpha").unwrap();
        // Independent trusted fixture transition advances the real biography.
        owner
            .commit_admitted_workset_revision(&root, &expected, &[(&retained, &alpha)])
            .unwrap();
        let current = owner.session.evidence().clone();
        let before = std::fs::read(root.join("body/biography.json")).unwrap();
        assert!(owner
            .commit_authorized_clock_revision(&root, approval)
            .unwrap_err()
            .contains("current owner basis"));
        assert_eq!(owner.session.evidence(), &current);
        assert_eq!(
            std::fs::read(root.join("body/biography.json")).unwrap(),
            before
        );
        assert!(owner.session.realization().is_none());
        std::fs::remove_dir_all(root).unwrap();
    }
}
