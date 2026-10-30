//! Root restart discovery through the ordinary installed Body-durable State.
use conduit_core as c;
use conduit_std_host::{
    ExternalForeDelivery, ExternalForeOutputAdapter, ExternalForeRun, RunControl,
    RunControlRequestId, StdHost, TimerAdapter,
};

#[derive(Debug)]
pub enum DurableSeedRecoveryRefusal {
    View,
    Type(c::StructuredInfoRefusal),
    Execution(String),
    Stop(String),
    MissingCurrent,
    ValueType,
    Representation,
    Reference(c::ResourceReferenceRefusal),
    Profile,
}

/// Execute a sealed read-only seed view. Root supplies actual Body/residence
/// custody and a current planned Host. A reference is data, never read authority.
/// This is preparation for subsequent catalogue/batch planning, not a resident
/// scheduler. Decode only after the ordinary kernel has acknowledged stopping.
pub fn recover_durable_catalog_seed<T: TimerAdapter>(
    host: &mut StdHost,
    body: &conduit_body::BodyId,
    residence_root: &std::path::Path,
    fragment: c::PlanFragment,
    timer: &mut T,
) -> Result<Option<c::BoundedResourceRef>, DurableSeedRecoveryRefusal> {
    use DurableSeedRecoveryRefusal as E;
    let value_type = c::optional_info_type(
        c::StructuredInfoType::leaf(c::kind_id(c::RESOURCE_REFERENCE_INFO_ID)).map_err(E::Type)?,
    )
    .map_err(E::Type)?;
    let value_kind = value_type.profile().map_err(E::Type)?.value_kind().clone();
    let ([state], [placement], [fore]) = (
        fragment.states.as_slice(),
        fragment.placements.as_slice(),
        fragment.fore_ports.as_slice(),
    ) else {
        return Err(E::View);
    };
    if state.lifetime != c::StateLifetime::Body
        || state.maximum_value_bytes != 1024
        || state.value_kind != value_kind
        || placement.kind_id.as_str() != c::STATE_VALUE_KIND
        || state.gear_id != placement.gear_id
        || fore.placement_id != placement.placement_id
        || fore.direction != c::PortDirection::Output
        || fore.temporal != c::PortTemporal::Current
        || fore.front_port_id.as_str() != "current"
        || fore.track != c::ConnectionTrack::Payload
        || fore.value_kind != value_kind
        || !fragment.connections.is_empty()
    {
        return Err(E::View);
    }
    let control = RunControl::default();
    let mut collector = SeedCurrent {
        value_kind,
        delivery: None,
        control: control.clone(),
        stop: Some(RunControlRequestId::new("recorder-seed-recovered").map_err(E::Stop)?),
    };
    let report = host
        .run_body_durable_external_plot_to(
            body,
            residence_root,
            fragment,
            ExternalForeRun {
                inputs: &[],
                output: &mut collector,
                sequential: false,
                stream: None,
            },
            &mut std::io::sink(),
            timer,
            &control,
        )
        .map_err(E::Execution)?;
    if !report.control_receipts.iter().any(|receipt| {
        receipt.request_id.as_str() == "recorder-seed-recovered"
            && receipt.disposition == conduit_std_host::RunControlDisposition::Accepted
    }) {
        return Err(E::Stop("missing accepted attributed stop".into()));
    }
    let delivery = collector.delivery.ok_or(E::MissingCurrent)?;
    let value = c::StructuredInfoValue::from_canonical_bytes(&delivery.bytes).map_err(E::Type)?;
    if value.value_type() != &value_type {
        return Err(E::ValueType);
    }
    match value.shape() {
        c::StructuredInfoValueShape::Variant { tag: "none", .. } => Ok(None),
        c::StructuredInfoValueShape::Variant {
            tag: "some",
            payload,
        } => {
            let c::StructuredInfoValueShape::Leaf(bytes) = payload.shape() else {
                return Err(E::Representation);
            };
            let reference = c::BoundedResourceRef::decode(bytes).map_err(E::Reference)?;
            if reference.content_profile.as_str() != "text/json-utf8@1"
                || reference.access_class.as_str() != "resource/snapshot@1"
                || reference.extent.items != Some(1)
                || reference.extent.bytes == 0
                || reference.extent.bytes > 4096
                || reference.lifetime.expires_at.is_some()
            {
                return Err(E::Profile);
            }
            Ok(Some(reference))
        }
        _ => Err(E::Representation),
    }
}
struct SeedCurrent {
    value_kind: c::KindId,
    delivery: Option<ExternalForeDelivery>,
    control: RunControl,
    stop: Option<RunControlRequestId>,
}
impl ExternalForeOutputAdapter for SeedCurrent {
    fn deliver(&mut self, delivery: ExternalForeDelivery) -> Result<(), String> {
        if delivery.front_port_id.as_str() != "current"
            || delivery.track != c::ConnectionTrack::Payload
            || delivery.value_kind != self.value_kind
            || delivery.bytes.len() > 1024
        {
            return Err("unexpected seed Current delivery".into());
        }
        if let Some(previous) = &self.delivery {
            if previous.bytes != delivery.bytes {
                return Err("read-only seed changed during recovery".into());
            }
            return Ok(());
        }
        self.delivery = Some(delivery);
        self.control
            .request_stop(self.stop.take().ok_or("seed stop already consumed")?)
            .map_err(|error| format!("stop seed recovery: {error:?}"))
    }
}
