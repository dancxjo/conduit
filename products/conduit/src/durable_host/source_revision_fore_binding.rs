//! Cold client bindings from original native selection and signed command bytes.
//! Hashes and parsing never replace membership, admission or live peer checks.
use super::*;
use conduit_std_host::pico_spawn::{ForeCarriageBounds, PreparedForeCarriage};
use sha2::{Digest, Sha256};
pub(super) struct FrozenFore {
    pub pin: c::AssignedLineForeRequirement,
    pub abnormal_kind: Option<c::KindId>,
}
/// Borrowed descriptive pins for a single authorized probe, never Ready/Start
/// evidence or permission to retire recorded observations.
#[allow(dead_code)] // Borrowed consumer metadata; Root decoder integration is separate.
pub(crate) struct ForeProbeBinding<'a> {
    pub(crate) body: &'a str,
    pub(crate) host: &'a str,
    pub(crate) boot: &'a str,
    pub(crate) generation: u64,
    pub(crate) plot: &'a ResidentPlot,
    pub(crate) image: &'a str,
    pub(crate) configuration: &'a str,
    pub(crate) usb_serial: &'a str,
    pub(crate) request: &'a c::AssignedLineForeRequirement,
    pub(crate) result: &'a c::AssignedLineForeRequirement,
    pub(crate) sessions: &'a [conduit_wire::ExternalForeSessionBinding; 2],
    pub(crate) original_approval_transcript: &'a [u8; 32],
}
struct OriginalProbeApproval {
    transcript: [u8; 32],
    bindings: [conduit_wire::ExternalForeSessionBinding; 2],
}
pub(in crate::durable_host::source_revision) struct DirectForeMaterial {
    body: String,
    host: String,
    boot: String,
    generation: u64,
    plot: ResidentPlot,
    image: String,
    configuration: String,
    usb_serial: String,
    approval: Option<OriginalProbeApproval>,
    bounds: conduit_host_make::fore_carrier::DeclaredForeCarrierBounds,
    rows: [FrozenFore; 2],
}
impl DirectForeMaterial {
    pub(super) fn new(
        policy: &DirectForeCarrierSelection,
        member: &owner::MemberPresentationBasis,
        configuration: String,
        bounds: conduit_host_make::fore_carrier::DeclaredForeCarrierBounds,
        mut rows: Vec<FrozenFore>,
    ) -> Result<Self, String> {
        if policy.installed_image_id.is_empty() || policy.installed_image_id.len() > 128 {
            return Err("installed direct carrier image identity is missing or unbounded".into());
        }
        rows.sort_by_key(|row| match row.pin.fore().direction() {
            c::AssignedForeDirection::Input => 0,
            c::AssignedForeDirection::Output => 1,
        });
        let rows: [FrozenFore; 2] = rows
            .try_into()
            .map_err(|_| "exact request/result Fore pair required")?;
        if rows[0].pin.fore().direction() != c::AssignedForeDirection::Input
            || rows[1].pin.fore().direction() != c::AssignedForeDirection::Output
        {
            return Err("direct carrier directions differ".into());
        }
        let actual = &member.host_advertisement;
        Ok(Self {
            body: member.credential.body_id.as_str().into(),
            host: actual.host_id.as_str().into(),
            boot: actual.boot_id.as_str().into(),
            generation: actual.offer_generation.0,
            plot: policy.plot.clone(),
            image: policy.installed_image_id.clone(),
            configuration,
            usb_serial: policy.usb_serial.clone(),
            approval: None,
            bounds,
            rows,
        })
    }
    pub(super) fn bindings(
        &self,
        payload: &[u8],
        custody: [u8; 32],
    ) -> Result<[conduit_wire::ExternalForeSessionBinding; 2], String> {
        let set = conduit_body::BodyWorksetDocumentView::<16>::decode(payload)
            .map_err(|e| format!("approved original workset: {e:?}"))?;
        let mut child = None;
        for index in 0..set.len() {
            let bytes = set.document(index).ok_or("approved child disappeared")?;
            let (meta, projection) = conduit_body::decode_body_fragment_document(bytes)
                .map_err(|e| format!("approved child: {e:?}"))?;
            if meta.source != self.plot.source_document_id.as_str()
                || meta.checked_plot != self.plot.checked_plot_id.as_str()
            {
                continue;
            }
            if meta.body != self.body
                || meta.host != self.host
                || meta.boot != self.boot
                || meta.offer_generation != self.generation
            {
                return Err("approved child differs from original native member scope".into());
            }
            if projection.get(8..10)
                != Some(
                    c::ASSIGNED_LINE_FORE_NATIVE_PLAN_SCHEMA
                        .to_le_bytes()
                        .as_slice(),
                )
            {
                return Err("original approved child does not carry schema-8 Lines".into());
            }
            if meta.fore_ports.len() % c::ASSIGNED_LOCAL_FORE_BYTES != 0
                || self.rows.iter().any(|row| {
                    meta.fore_ports
                        .as_chunks::<{ c::ASSIGNED_LOCAL_FORE_BYTES }>()
                        .0
                        .iter()
                        .filter(|pin| pin.as_slice() == row.pin.fore().payload())
                        .count()
                        != 1
                })
            {
                return Err("approved child lacks exact original Fore inventory".into());
            }
            if child.replace(bytes).is_some() {
                return Err("original native child scope is ambiguous".into());
            }
        }
        let child = child.ok_or("original selected native child absent")?;
        let b = self.bounds;
        let full = conduit_wire::fore_carrier_scope_commitment(
            &Sha256::digest(child).into(),
            &self.image,
            &self.configuration,
            conduit_wire::ForeCarrierScopeBounds {
                maximum_lines: b.maximum_lines,
                maximum_frame_bytes: b.maximum_frame_bytes,
                maximum_payload_bytes: b.maximum_payload_bytes,
                maximum_buffered_bytes: b.maximum_buffered_bytes as u64,
                maximum_in_flight_items: b.maximum_in_flight_items,
                maximum_frames: b.maximum_frames,
            },
        );
        Ok(core::array::from_fn(|i| {
            conduit_wire::ExternalForeSessionBinding {
                scope_commitment: conduit_wire::external_fore_scope_commitment(
                    &full,
                    &custody,
                    self.rows[i].pin.payload(),
                ),
                limits: conduit_wire::SessionLimits {
                    maximum_in_flight_items: b.maximum_in_flight_items,
                    maximum_payload_bytes: b.maximum_payload_bytes,
                    maximum_buffered_bytes: b.maximum_buffered_bytes as u32,
                },
                maximum_frame_bytes: b.maximum_frame_bytes,
                abnormal_kind: self.rows[i].abnormal_kind.clone(),
            }
        }))
    }
    pub(in crate::durable_host::source_revision) fn prepare_carriage(
        &mut self,
        payload: &[u8],
        custody: [u8; 32],
    ) -> Result<PreparedForeCarriage, String> {
        if self.approval.is_some() {
            return Err("original Fore approval already retained; replacement refused".into());
        }
        let bindings = self.bindings(payload, custody)?;
        let b = self.bounds;
        let prepared = PreparedForeCarriage::prepare(
            bindings.clone(),
            ForeCarriageBounds {
                maximum_frames: b.maximum_frames,
                maximum_payload_bytes: b.maximum_payload_bytes,
                maximum_frame_bytes: b.maximum_frame_bytes,
                maximum_buffer_bytes: b.maximum_buffered_bytes,
            },
        )
        .map_err(|e| format!("prepare original native client: {e:?}"))?;
        self.approval = Some(OriginalProbeApproval {
            transcript: custody,
            bindings,
        });
        Ok(prepared)
    }
    pub(in crate::durable_host::source_revision) fn approval_transcript(
        &self,
    ) -> Result<&[u8; 32], String> {
        self.approval
            .as_ref()
            .map(|approval| &approval.transcript)
            .ok_or_else(|| "original Fore approval custody absent".into())
    }
    /// Recompute from the actual Owner's still-retained command, not a report.
    pub(in crate::durable_host::source_revision) fn verify_probe_binding(
        &self,
        payload: &[u8],
        transcript: &[u8; 32],
    ) -> Result<ForeProbeBinding<'_>, String> {
        let approval = self
            .approval
            .as_ref()
            .ok_or("original Fore approval custody absent")?;
        if transcript != &approval.transcript
            || self.bindings(payload, *transcript)? != approval.bindings
        {
            return Err("probe differs from original approved BoundFore custody".into());
        }
        Ok(ForeProbeBinding {
            body: &self.body,
            host: &self.host,
            boot: &self.boot,
            generation: self.generation,
            plot: &self.plot,
            image: &self.image,
            configuration: &self.configuration,
            usb_serial: &self.usb_serial,
            request: &self.rows[0].pin,
            result: &self.rows[1].pin,
            sessions: &approval.bindings,
            original_approval_transcript: &approval.transcript,
        })
    }
}
