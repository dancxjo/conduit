//! Typed, bounded observation endpoints for the authored appliance. A publication
//! is committed semantic state, not a second scheduler, a store or a Face Mask.
use alloc::{string::String, vec};
use conduit_core as c;
use conduit_plot::{KindSignature, ProfileCatalog, StartupCatalog};
mod back;
pub use back::{PublicationBack, PublicationSnapshot};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublicationChannel {
    Reading,
    CaptureTimeout,
    ProviderFailure,
    GeometryRefusal,
}
impl PublicationChannel {
    pub const ALL: [Self; 4] = [
        Self::Reading,
        Self::CaptureTimeout,
        Self::ProviderFailure,
        Self::GeometryRefusal,
    ];
    pub const fn kind_id(self) -> &'static str {
        match self {
            Self::Reading => "environment/observe-dht11",
            Self::CaptureTimeout => "environment/observe-capture-timeout",
            Self::ProviderFailure => "environment/observe-provider-failure",
            Self::GeometryRefusal => "environment/observe-geometry-refusal",
        }
    }
    pub const fn index(self) -> usize {
        match self {
            Self::Reading => 0,
            Self::CaptureTimeout => 1,
            Self::ProviderFailure => 2,
            Self::GeometryRefusal => 3,
        }
    }
    pub const fn maximum_bytes(self) -> usize {
        if matches!(self, Self::Reading) {
            4096
        } else {
            1024
        }
    }
}

/// Checked contracts describe meaning. Their construction installs no provider
/// and makes no Host/Boot, GPIO or storage-availability claim.
pub struct PublicationContract {
    channel: PublicationChannel,
    value_type: c::StructuredInfoType,
    kind: c::Kind,
}
impl PublicationContract {
    pub fn prepare(channel: PublicationChannel) -> Result<Self, c::StructuredInfoRefusal> {
        let value_type = match channel {
            PublicationChannel::Reading => super::observation::ObservationContract::prepare()?
                .result_type()
                .clone(),
            PublicationChannel::CaptureTimeout => c::StructuredInfoType::from_canonical_bytes(
                include_bytes!(concat!(env!("OUT_DIR"), "/pulse-capture.type")),
            )?,
            PublicationChannel::ProviderFailure | PublicationChannel::GeometryRefusal => {
                c::StructuredInfoType::leaf(c::kind_id(c::EMPTY_INFO_ID))?
            }
        };
        let kind = c::Kind {
            kind_id: c::kind_id(channel.kind_id()),
            kind_contract_revision: "environment/observation-publication@1".into(),
            startup_parameters: vec![],
            shorthand: None,
            inputs: vec![c::PortDescriptor {
                port_id: c::port_id("value"),
                value_kind: value_type.profile()?.value_kind().clone(),
                direction: c::PortDirection::Input,
                temporal: c::PortTemporal::Flow { closes: true },
                abnormal_kind: None,
            }],
            outputs: vec![],
            configuration: vec![],
            semantic_laws: vec![c::KindSemanticLaw::Terminal(
                c::KindTerminalBehavior::CompletesWhenInputsClose,
            )],
            limits: c::CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: channel.maximum_bytes() as u32,
            },
        };
        Ok(Self {
            channel,
            value_type,
            kind,
        })
    }
    pub fn kind(&self) -> &c::Kind {
        &self.kind
    }
    pub fn value_type(&self) -> &c::StructuredInfoType {
        &self.value_type
    }
    pub fn channel(&self) -> PublicationChannel {
        self.channel
    }
    pub fn install(
        &self,
        startup: &mut StartupCatalog,
        profiles: &mut ProfileCatalog,
    ) -> Result<(), String> {
        startup.insert(KindSignature {
            kind: self.channel.kind_id().into(),
            startup_parameters: vec![],
        })?;
        startup.insert_fore(self.channel.kind_id(), self.kind.checked_front())?;
        profiles
            .insert_kind(self.kind.clone())
            .map_err(|e| alloc::format!("publication contract: {e:?}"))
    }
}
