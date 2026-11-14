//! Shared native presentation on the installed Seed's actual initialized Host.
//! Initial admission and current-member presentation retain distinct authority.
use super::{RuntimeStatus, wire};
use crate::durable_host::routed_join::MaskSelection;
use crate::durable_host_control::DurableHostRuntime;
use conduit_linux_display::Scanout;
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

mod input;

const REFRESH_PERIOD: Duration = Duration::from_secs(10);
const MAXIMUM_REFRESHES: u8 = 4;

pub(in crate::durable_host) struct NativePresentation {
    owner: Option<NativeOwner>,
    next_refresh: Instant,
    refreshes: u8,
    play_sequence: u64,
    state: PathBuf,
    status: RuntimeStatus,
    reconnect_delay: Option<Duration>,
    current_input_route: Option<conduit_face::RemoteOwnerMaskRouteSeal>,
    pending_input: Option<conduit_std_host::SelectedNativeInteraction>,
    input_sequence: u64,
    primary_pressed: bool,
}

// Retain the original admitted route and its custody inline across input dispatch.
#[allow(clippy::large_enum_variant)]
enum NativeOwner {
    Joining(wire::OwnerRoute),
    Member(wire::member_carrier_client::MemberRoute),
}
impl NativeOwner {
    fn acknowledge(&self, show: &conduit_face::MaskShow) -> Result<(), String> {
        match self {
            Self::Joining(route) => route.acknowledge(show),
            Self::Member(route) => route.acknowledge(show),
        }
    }
    fn refresh(&mut self) -> Result<wire::Snapshot, String> {
        match self {
            Self::Joining(route) => route.refresh(),
            Self::Member(route) => route.refresh(),
        }
    }
    fn poll_presence(&mut self) -> Result<(), String> {
        match self {
            Self::Joining(_) => Ok(()),
            Self::Member(route) => route.poll_presence(),
        }
    }
}
impl conduit_std_host::NativeMaskEncounter for NativeOwner {
    fn validate_current(
        &self,
        face: &conduit_face::Face,
        route: &conduit_face::RemoteOwnerMaskRouteSeal,
        local: &conduit_core::HostAdvertisement,
    ) -> Result<(), String> {
        match self {
            Self::Joining(owner) => owner.validate_current(face, route, local),
            Self::Member(owner) => owner.validate_current(face, route, local),
        }
    }
}

impl NativePresentation {
    pub(in crate::durable_host) fn join_seed(
        state: &Path,
        status: &mut RuntimeStatus,
        runtime: &mut DurableHostRuntime,
        output: &mut Scanout<'_>,
    ) -> Result<Self, String> {
        if runtime
            .presentation_host_mut()?
            .ready_selected_native_mask()
            .is_none()
        {
            return Err("native presentation has no initialized selected Mask".into());
        }
        let installation = super::super::read_installation(&state.join("installation.json"))?;
        let reconnect_delay = crate::durable_host::routed_join::reconnect_delay(state)?;
        let connection = if installation.joined_body_state.is_some() {
            member_connection(runtime, status, state)
        } else {
            let invitation = super::super::pending_seed::original_invitation(state)?;
            wire::OwnerRoute::admit_initialized(
                runtime,
                status,
                state,
                invitation,
                MaskSelection::Native,
            )
            .map(|(route, snapshot)| (NativeOwner::Joining(route), snapshot))
        };
        let mut current_input_route = None;
        let owner = match connection {
            Ok((owner, snapshot)) => {
                let (acknowledged, route) = present(runtime, output, &owner, snapshot, 1)?;
                current_input_route = route;
                acknowledged.then_some(owner)
            }
            Err(error) if reconnect_delay.is_some() => {
                // The retained contact is operator-selected planning policy,
                // not a current owner, route, Face or executable permission.
                eprintln!("native member contact unavailable; no current Show: {error}");
                None
            }
            Err(error) => return Err(error),
        };
        let next_delay = if owner.is_some() {
            REFRESH_PERIOD
        } else {
            reconnect_delay.unwrap_or(REFRESH_PERIOD)
        };
        Ok(Self {
            owner,
            next_refresh: Instant::now() + next_delay,
            refreshes: 0,
            play_sequence: 1,
            state: state.into(),
            status: RuntimeStatus {
                schema: status.schema.clone(),
                host_id: status.host_id.clone(),
                boot_id: status.boot_id.clone(),
                offer_generation: status.offer_generation,
                process_id: status.process_id,
                release_bundle_sha256: status.release_bundle_sha256.clone(),
                body_id: status.body_id.clone(),
            },
            reconnect_delay,
            current_input_route,
            pending_input: None,
            input_sequence: 0,
            primary_pressed: false,
        })
    }

    fn reconnect(
        &mut self,
        runtime: &mut DurableHostRuntime,
        output: &mut Scanout<'_>,
    ) -> Result<(), String> {
        let delay = self
            .reconnect_delay
            .ok_or("native continuing contact absent")?;
        let connection = member_connection(runtime, &mut self.status, &self.state);
        self.next_refresh = Instant::now() + delay;
        match connection {
            Ok((owner, snapshot)) => {
                self.play_sequence = self
                    .play_sequence
                    .checked_add(1)
                    .ok_or("native child Play sequence exhausted")?;
                // Kernel/output failures remain terminal. Only an unavailable
                // authenticated contact is retried by this explicit policy.
                let (acknowledged, route) =
                    present(runtime, output, &owner, snapshot, self.play_sequence)?;
                self.current_input_route = route;
                if acknowledged {
                    self.owner = Some(owner);
                    self.next_refresh = Instant::now() + REFRESH_PERIOD;
                }
            }
            Err(error) => eprintln!(
                "native member contact unavailable; previous pixels are historical: {error}"
            ),
        }
        Ok(())
    }

    pub(in crate::durable_host) fn poll(
        &mut self,
        runtime: &mut DurableHostRuntime,
        output: &mut Scanout<'_>,
    ) -> Result<(), String> {
        if self.reconnect_delay.is_some()
            && matches!(&self.owner, Some(NativeOwner::Member(route)) if route.renewal_due())
        {
            // Retire the old carrier before allocating/authenticating the new
            // one. There is never overlap or an enlarged presence lease.
            self.retire_input(runtime)?;
            drop(self.owner.take());
            self.next_refresh = Instant::now();
        }
        if self.owner.is_none() {
            if self.reconnect_delay.is_some() && Instant::now() >= self.next_refresh {
                self.reconnect(runtime, output)?;
            }
            return Ok(());
        }
        if let Some(owner) = self.owner.as_mut() {
            if let Err(error) = owner.poll_presence() {
                self.retire_input(runtime)?;
                self.owner = None;
                self.next_refresh = Instant::now() + self.reconnect_delay.unwrap_or(REFRESH_PERIOD);
                eprintln!("native carrier lost; displayed Show is historical: {error}");
                return Ok(());
            }
        }
        if self.pending_input.is_some() {
            let outcome = match &self.owner {
                Some(NativeOwner::Member(owner)) => owner.poll_action(),
                _ => Err("pending native action lost its member carrier".into()),
            };
            let outcome = match outcome {
                Ok(outcome) => outcome,
                Err(error) => {
                    self.retire_input(runtime)?;
                    self.owner = None;
                    self.next_refresh =
                        Instant::now() + self.reconnect_delay.unwrap_or(REFRESH_PERIOD);
                    eprintln!("native action response lost; no replay:{error}");
                    return Ok(());
                }
            };
            if let Some(outcome) = outcome {
                self.finish_input(runtime)?;
                if let Err(error) = outcome {
                    eprintln!("native owner action refused:{error}");
                }
                self.next_refresh = Instant::now();
            } else {
                return Ok(());
            }
        }
        if self.owner.is_none() || Instant::now() < self.next_refresh {
            return Ok(());
        }
        if matches!(&self.owner, Some(NativeOwner::Joining(_)))
            && self.refreshes == MAXIMUM_REFRESHES
        {
            // Never turn an expired/consumed finite transport grant into another
            // invitation or implicit execution authority. Existing pixels are a
            // historical Show; no new Show or freshness assertion is published.
            // The Host and output remain alive. Exhausting this presentation
            // encounter retires only its bearer, not the member's attachment.
            // An actual departure requires a separate explicit lifecycle act.
            self.retire_input(runtime)?;
            drop(self.owner.take());
            eprintln!("native owner encounter ended: displayed Show is historical");
            return Ok(());
        }
        let owner = self.owner.as_mut().ok_or("native owner encounter absent")?;
        let snapshot = match owner.refresh() {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.retire_input(runtime)?;
                self.owner = None;
                self.next_refresh = Instant::now() + self.reconnect_delay.unwrap_or(REFRESH_PERIOD);
                eprintln!("native owner encounter lost; displayed Show is historical: {error}");
                return Ok(());
            }
        };
        self.play_sequence = self
            .play_sequence
            .checked_add(1)
            .ok_or("native child Play sequence exhausted")?;
        retire_frame(runtime)?;
        self.current_input_route = None;
        let (owner_acknowledged, route) =
            present(runtime, output, owner, snapshot, self.play_sequence)?;
        self.current_input_route = route;
        if !owner_acknowledged {
            self.owner = None;
        }
        self.refreshes = self.refreshes.saturating_add(1);
        self.next_refresh = Instant::now() + REFRESH_PERIOD;
        Ok(())
    }
}

fn member_connection(
    runtime: &mut DurableHostRuntime,
    status: &mut RuntimeStatus,
    state: &Path,
) -> Result<(NativeOwner, wire::Snapshot), String> {
    let encounter = match crate::durable_host::routed_join::load_and_resolve_member_contact(state)?
    {
        Some(fresh) => fresh,
        None => wire::load_member_encounter(state)?,
    };
    wire::member_carrier_client::MemberRoute::connect_initialized(runtime, status, state, encounter)
        .map(|(route, snapshot)| (NativeOwner::Member(route), snapshot))
}

fn present(
    runtime: &mut DurableHostRuntime,
    output: &mut Scanout<'_>,
    owner: &NativeOwner,
    snapshot: wire::Snapshot,
    play_sequence: u64,
) -> Result<(bool, Option<conduit_face::RemoteOwnerMaskRouteSeal>), String> {
    let front = snapshot
        .face
        .subjects
        .first()
        .ok_or("native owner Face has no front subject")?
        .identity
        .as_str();
    let host = runtime.presentation_host_mut()?;
    let interactive = host.selected_native_mask_supports_input();
    if interactive && !snapshot.interactions_admitted {
        return Err("owner did not admit native input for this Show".into());
    }
    let frame = if interactive {
        host.render_selected_interactive_native_mask(
            owner,
            &snapshot.route,
            &snapshot.face,
            play_sequence,
            front,
            output,
        )?
    } else {
        host.render_selected_native_mask(
            owner,
            &snapshot.route,
            &snapshot.face,
            play_sequence,
            front,
            output,
        )?
    };
    // Acknowledge the actual Available Show only after the physical driver ACK.
    let acknowledgement = owner.acknowledge(
        host.selected_native_mask_show()
            .ok_or("native Available Show absent")?,
    );
    println!(
        "native Mask displayed Face {} revision {} on frame {}",
        snapshot.face.identity.as_str(),
        frame.face_revision,
        frame.scanout_frame_sequence
    );
    if !interactive {
        retire_frame(runtime)?;
    }
    match acknowledgement {
        Ok(()) => {
            println!(
                "native owner accepted Show for Face {} revision {} on frame {}",
                snapshot.face.identity.as_str(),
                frame.face_revision,
                frame.scanout_frame_sequence
            );
            Ok((true, interactive.then_some(snapshot.route)))
        }
        Err(error) => {
            // Physical commit is real even if its return is lost or refused.
            // End this encounter; no further Show/action uses its grant. Keep
            // the actual Host/output alive with explicitly historical pixels.
            eprintln!("native Show is historical; owner acknowledgment was not accepted: {error}");
            retire_frame(runtime)?;
            Ok((false, None))
        }
    }
}

// Dispose completed local Mask custody. This is never a durable recording ACK.
fn retire_frame(runtime: &mut DurableHostRuntime) -> Result<(), String> {
    let host = runtime.presentation_host_mut()?;
    let mut evidence = host.take_selected_native_mask_evidence()?;
    if evidence.is_some() {
        host.retire_selected_native_mask_evidence(&mut evidence)?;
    }
    Ok(())
}
