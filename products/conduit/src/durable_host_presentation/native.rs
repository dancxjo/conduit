//! Read-only native presentation on the installed Seed's actual initialized Host.
//! This consumes the existing finite owner encounter; it is not a reconnect grant.
use super::{wire, RuntimeStatus};
use crate::durable_host::routed_join::MaskSelection;
use crate::durable_host_control::DurableHostRuntime;
use conduit_linux_display::Scanout;
use std::{
    path::Path,
    time::{Duration, Instant},
};

const REFRESH_PERIOD: Duration = Duration::from_secs(10);
const MAXIMUM_REFRESHES: u8 = 4;

pub(crate) struct NativePresentation {
    owner: Option<wire::OwnerRoute>,
    next_refresh: Instant,
    refreshes: u8,
    play_sequence: u64,
}

impl NativePresentation {
    pub(crate) fn join_seed(
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
        let invitation = super::super::pending_seed::original_invitation(state)?;
        let (owner, snapshot) = wire::OwnerRoute::admit_initialized(
            runtime,
            status,
            state,
            invitation,
            MaskSelection::Native,
        )?;
        present(runtime, output, &owner, snapshot, 1)?;
        Ok(Self {
            owner: Some(owner),
            next_refresh: Instant::now() + REFRESH_PERIOD,
            refreshes: 0,
            play_sequence: 1,
        })
    }

    pub(crate) fn poll(
        &mut self,
        runtime: &mut DurableHostRuntime,
        output: &mut Scanout<'_>,
    ) -> Result<(), String> {
        if self.owner.is_none() || Instant::now() < self.next_refresh {
            return Ok(());
        }
        if self.refreshes == MAXIMUM_REFRESHES {
            // Never turn an expired/consumed finite transport grant into another
            // invitation or implicit execution authority. Existing pixels are a
            // historical Show; no new Show or freshness assertion is published.
            self.owner = None;
            eprintln!("native owner encounter ended: displayed Show is historical");
            return Ok(());
        }
        let owner = self.owner.as_mut().ok_or("native owner encounter absent")?;
        let snapshot = match owner.refresh() {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.owner = None;
                eprintln!("native owner encounter lost; displayed Show is historical: {error}");
                return Ok(());
            }
        };
        self.play_sequence = self
            .play_sequence
            .checked_add(1)
            .ok_or("native child Play sequence exhausted")?;
        present(runtime, output, owner, snapshot, self.play_sequence)?;
        self.refreshes += 1;
        self.next_refresh = Instant::now() + REFRESH_PERIOD;
        Ok(())
    }
}

fn present(
    runtime: &mut DurableHostRuntime,
    output: &mut Scanout<'_>,
    owner: &wire::OwnerRoute,
    snapshot: wire::Snapshot,
    play_sequence: u64,
) -> Result<(), String> {
    let front = snapshot
        .face
        .subjects
        .first()
        .ok_or("native owner Face has no front subject")?
        .identity
        .as_str();
    let host = runtime.presentation_host_mut()?;
    let frame = host.render_selected_native_mask(
        owner,
        &snapshot.route,
        &snapshot.face,
        play_sequence,
        front,
        output,
    )?;
    // Acknowledge the actual Available Show only after the physical driver ACK.
    let acknowledgement = owner.acknowledge(
        host.selected_native_mask_show()
            .ok_or("native Available Show absent")?,
    );
    let mut evidence = host.take_selected_native_mask_evidence()?;
    let retained = evidence.as_ref().ok_or("native retired evidence absent")?;
    println!(
        "native Mask displayed Face {} revision {} on frame {}; pixels {}",
        retained.composition().face_id.as_str(),
        frame.face_revision,
        frame.scanout_frame_sequence,
        retained.evidence().receipt.scanout_pixels_written
    );
    // This disposes local completed-Play diagnostics, never recorded observations
    // or a recorder's durable acknowledgment. The exact provider/ledger checks
    // remain inside StdHost; no resource charge is silently cleared here.
    host.retire_selected_native_mask_evidence(&mut evidence)?;
    acknowledgement
}
