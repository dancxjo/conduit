//! Incremental Cord transfer estimates, separate from operation fulfillment.
//! Binding preserves already-admitted attribution; it does not authenticate a
//! producer, admit a Line, or turn a modeled duration into a deadline.
use super::{
    CostCalibrationRevision, CostDurationRange, CostEvaluationContext, CostEvidenceRefusal,
    CostObservationWindow, MAXIMUM_COST_ID_BYTES,
};
use conduit_core::{HostAdvertisement, PlannedConnection, PlannedGear, SignId};

/// One finite encoded transfer workload. These are encoded bytes and item
/// counts, never a proxy for compute work or a semantic content length.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CostTransferExtent {
    pub encoded_bytes: u32,
    pub items: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CostTransferRefusal {
    Evidence(CostEvidenceRefusal),
    EndpointChanged,
    ConnectionChanged,
    ExtentChanged,
    ExtentLimit,
    MissingCalibration,
    ProducerChanged,
    MissingSelectedLine,
}

/// Exact immutable Cord/Line and endpoint snapshots for a supplied estimate.
/// Local Cords also need explicit evidence; local placement does not imply a
/// zero duration. The range must cover only incremental transfer, excluding
/// setup, queue and service stages already included in operation forecasts.
#[derive(Debug)]
pub struct CostTransferForecast<'a> {
    connection: &'a PlannedConnection,
    source: &'a HostAdvertisement,
    sink: &'a HostAdvertisement,
    extent: CostTransferExtent,
    sign: &'a SignId,
    calibration: CostCalibrationRevision<'a>,
    window: CostObservationWindow,
    duration: CostDurationRange,
}
impl<'a> CostTransferForecast<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn bind(
        connection: &'a PlannedConnection,
        source_placement: &PlannedGear,
        sink_placement: &PlannedGear,
        source: &'a HostAdvertisement,
        sink: &'a HostAdvertisement,
        extent: CostTransferExtent,
        sign: &'a SignId,
        calibration: CostCalibrationRevision<'a>,
        window: CostObservationWindow,
        duration: CostDurationRange,
    ) -> Result<Self, CostTransferRefusal> {
        use CostTransferRefusal as R;
        if connection.source_placement_id != source_placement.placement_id
            || connection.sink_placement_id != sink_placement.placement_id
            || source_placement.host_id != source.host_id
            || source_placement.boot_id != source.boot_id
            || source_placement.offer_generation != source.offer_generation
            || sink_placement.host_id != sink.host_id
            || sink_placement.boot_id != sink.boot_id
            || sink_placement.offer_generation != sink.offer_generation
            || !source_placement.outputs.iter().any(|port| {
                port.port_id == connection.source_port_id
                    && port.value_kind == connection.value_kind
            })
            || !sink_placement.inputs.iter().any(|port| {
                port.port_id == connection.sink_port_id && port.value_kind == connection.value_kind
            })
        {
            return Err(R::EndpointChanged);
        }
        if extent.items == 0
            || extent.items > connection.item_capacity
            || extent.encoded_bytes > connection.byte_capacity
        {
            return Err(R::ExtentLimit);
        }
        let remote = source.host_id != sink.host_id;
        match &connection.selected_line {
            Some(line) if remote => {
                if line.binding.source.host_id != source.host_id
                    || line.binding.source.boot_id != source.boot_id
                    || line.binding.sink.host_id != sink.host_id
                    || line.binding.sink.boot_id != sink.boot_id
                    || !connection.admitted_lines.contains(line)
                {
                    return Err(R::EndpointChanged);
                }
            }
            None if !remote && source.boot_id == sink.boot_id => {}
            _ => return Err(R::MissingSelectedLine),
        }
        if sign.as_str().is_empty() || sign.as_str().len() > MAXIMUM_COST_ID_BYTES {
            return Err(R::Evidence(CostEvidenceRefusal::InvalidIdentity));
        }
        if window.observed().clock().host_id() != &source.host_id
            || window.observed().clock().boot_id() != &source.boot_id
        {
            return Err(R::EndpointChanged);
        }
        Ok(Self {
            connection,
            source,
            sink,
            extent,
            sign,
            calibration,
            window,
            duration,
        })
    }

    /// Revalidate the selected route, both endpoint inventories, workload and
    /// source-clock freshness against independently supplied current facts.
    /// A fresh source sample cannot make a changed sink Boot current.
    pub fn validate_at(
        &self,
        connection: &PlannedConnection,
        extent: CostTransferExtent,
        source: &CostEvaluationContext<'_>,
        sink: &HostAdvertisement,
    ) -> Result<CostDurationRange, CostTransferRefusal> {
        use CostTransferRefusal as R;
        if connection != self.connection {
            return Err(R::ConnectionChanged);
        }
        if extent != self.extent {
            return Err(R::ExtentChanged);
        }
        if source.host != self.source || sink != self.sink {
            return Err(R::ProducerChanged);
        }
        if !source.calibrations.contains(&self.calibration) {
            return Err(R::MissingCalibration);
        }
        self.window.validate_at(source.now).map_err(R::Evidence)?;
        Ok(self.duration)
    }
    pub fn connection(&self) -> &'a PlannedConnection {
        self.connection
    }
    pub fn extent(&self) -> CostTransferExtent {
        self.extent
    }
    pub fn sign(&self) -> &'a SignId {
        self.sign
    }
    pub fn source(&self) -> &'a HostAdvertisement {
        self.source
    }
    pub fn sink(&self) -> &'a HostAdvertisement {
        self.sink
    }
}
