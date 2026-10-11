//! Class admission consumes an exact frozen native observation through pure Source.
use super::usb_request::{UsbPreflightBootRequest, UsbProtocolBootRequest};
use crate::{arch::UsbConfigurationSnapshot, protocol_source::PreparedProtocolEntry};
use alloc::{boxed::Box, collections::BTreeMap, vec, vec::Vec};
use conduit_composite::{
    KernelCompositeSignStorage, KernelCompositeStatus, KernelCompositeTerminal,
};
use conduit_core::*;
use conduit_planner::PlanningOptions;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum DiscoveryVerdict {
    Supported,
    Unsupported,
}

pub(super) struct RetiredUsbDiscovery {
    verdict: DiscoveryVerdict,
    snapshot: UsbConfigurationSnapshot,
    package_digest: [u8; 32],
    root_digest: [u8; 32],
    class_resident: conduit_body::ResidentPlot,
    // Retain the exact pure Plan/Play and complete Sign, not just a verdict.
    _play: crate::pure_protocol_play::PreparedPureProtocolPlay,
}
impl RetiredUsbDiscovery {
    pub(super) fn supported(&self) -> bool {
        self.verdict == DiscoveryVerdict::Supported
    }
    pub(super) fn validate_class(&self, entry: &PreparedProtocolEntry) -> Result<(), &'static str> {
        if !self.supported() {
            return Err("protocol-usb-profile-unsupported");
        }
        if entry.package_digest() != &self.package_digest || entry.resident() != self.class_resident
        {
            return Err("protocol-usb-preflight-class-substitution");
        }
        Ok(())
    }
    pub(super) fn validate_request(
        &self,
        request: &UsbProtocolBootRequest,
    ) -> Result<(), &'static str> {
        if carrier::request_digest(request)? != self.root_digest {
            return Err("protocol-usb-preflight-root-substitution");
        }
        Ok(())
    }
    pub(super) fn validate_attachment(
        &self,
        controller: &crate::arch::XhciReady,
        device: &crate::arch::UsbDevice,
    ) -> Result<(), &'static str> {
        self.snapshot
            .validate(controller, device)
            .map_err(|_| "protocol-usb-discovery-stale")
    }
}

pub(super) fn accept(
    package: &[u8],
    request: &UsbProtocolBootRequest,
    selection: &UsbPreflightBootRequest,
    snapshot: UsbConfigurationSnapshot,
    host: &HostId,
    boot: &BootId,
) -> Result<Box<RetiredUsbDiscovery>, &'static str> {
    request
        .bind_package(package)
        .map_err(|_| "protocol-usb-preflight-package")?;
    let class = PreparedProtocolEntry::prepare(package, &request.entry)
        .map_err(|_| "protocol-usb-preflight-class-source")?;
    let class_resident = class.resident();
    let package_digest = *class.package_digest();
    let entry = PreparedProtocolEntry::prepare(package, &selection.entry)
        .map_err(|_| "protocol-usb-preflight-source")?;
    // The immutable package digest binds both preflight and retained class entry.
    // Source/check identities remain in the admitted artifact and its Sign.
    let fore = &entry.expanded().front;
    if fore.inputs().len() != 1 || fore.outputs().len() != 1 {
        return Err("protocol-usb-preflight-fore");
    }
    let input = fore.inputs()[0].clone();
    let output = fore.outputs()[0].clone();
    let contract = crate::usb_base::control_contract::ControlContract::prepare()
        .map_err(|_| "protocol-usb-preflight-contract")?;
    if output.value_kind != kind_id("value/bool") {
        return Err("protocol-usb-preflight-fore");
    }
    let schema = entry
        .input_schema(&input.port_id)
        .ok_or("protocol-usb-preflight-fore")?;
    let root_digest = carrier::request_digest(request)?;
    let value = ValuePayload {
        value_kind: input.value_kind.clone(),
        encoded: carrier::encode(&schema, &snapshot, request, &contract)?,
    };
    let mut advertisement = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: host.clone(),
        boot_id: boot.clone(),
        offer_generation: OfferGeneration(1),
        profile: "conduitos/pure-discovery@1".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    entry
        .publish_pure_backs(&mut advertisement)
        .map_err(|_| "protocol-usb-preflight-offers")?;
    let hosts = [advertisement];
    let placements = entry
        .placements(&hosts)
        .map_err(|_| "protocol-usb-preflight-placement")?;
    let empty = BTreeMap::new();
    let artifact = entry
        .plan(
            &hosts,
            &placements,
            &[BaseImplementationId::from("conduit.base/local@1")],
            PlanningOptions {
                connection_bases: &empty,
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: 1,
                connection_byte_capacity: 4096,
                authority_grants: &[],
                protected_resource_grants: &[],
                line_offers: &[],
            },
        )
        .map_err(|_| "protocol-usb-preflight-plan")?;
    let play = artifact
        .prepare_pure(KernelCompositeSignStorage::default())
        .map_err(|_| "protocol-usb-preflight-prepare")?;
    let mut result = ValuePayload {
        value_kind: output.value_kind.clone(),
        encoded: Vec::with_capacity(64),
    };
    // Every retained allocation precedes pure Play, including negative history.
    let mut retained = Box::new(RetiredUsbDiscovery {
        snapshot,
        package_digest,
        root_digest,
        class_resident,
        verdict: DiscoveryVerdict::Unsupported,
        _play: play,
    });
    trace::prepared(&retained, &value)?;
    let play = &mut retained._play;
    let mut verdict = None;
    let mut closed = false;
    play.start().map_err(|_| "protocol-usb-preflight-start")?;
    if !matches!(
        play.admit_input(&input.port_id, 0, &value)
            .map_err(|_| "protocol-usb-preflight-input")?,
        conduit_kernel::scheduler::RemoteIngressOutcome::Accepted { .. }
    ) {
        return Err("protocol-usb-preflight-input-pressure");
    }
    play.close_input(&input.port_id)
        .map_err(|_| "protocol-usb-preflight-close")?;
    for _ in 0..selection.maximum_steps {
        let status = play.step().map_err(|_| "protocol-usb-preflight-step")?;
        while let Some(sequence) = play
            .output_into(&output.port_id, &mut result)
            .map_err(|_| "protocol-usb-preflight-output")?
        {
            validate_primitive_info(output.value_kind.as_str(), &result.encoded)
                .map_err(|_| "protocol-usb-preflight-verdict")?;
            if verdict.is_some() {
                return Err("protocol-usb-preflight-multiple-verdicts");
            }
            verdict = Some(result.encoded == [1]);
            trace::output(&output.port_id, &result)?;
            // False is an admitted pure observation: acknowledge it and retain
            // Normal retirement before refusing the physical class profile.
            play.complete_output(&output.port_id, sequence)
                .map_err(|_| "protocol-usb-preflight-output")?;
        }
        if let Some(terminal) = play
            .output_terminal_into(&output.port_id, &mut result)
            .map_err(|_| "protocol-usb-preflight-terminal")?
        {
            if terminal != KernelCompositeTerminal::Normal {
                return Err("protocol-usb-preflight-terminal");
            }
            closed = true;
        }
        if status == KernelCompositeStatus::Complete && closed {
            retained.verdict = match verdict {
                Some(true) => DiscoveryVerdict::Supported,
                Some(false) => DiscoveryVerdict::Unsupported,
                None => return Err("protocol-usb-preflight-no-verdict"),
            };
            trace::retired(&retained.verdict)?;
            return Ok(retained);
        }
    }
    Err("protocol-usb-preflight-budget")
}

#[cfg(test)]
#[path = "usb_preflight/tests.rs"]
mod tests;

#[path = "usb_preflight/trace.rs"]
mod trace;

#[path = "usb_preflight/carrier.rs"]
mod carrier;
