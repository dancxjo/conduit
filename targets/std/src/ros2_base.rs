//! Narrow ROS 2 topic Base. ROS graph facts remain realization data.

use conduit_core::{
    BaseCapabilityHandle, BaseCapabilityRefusal, BaseCapabilityTable, BaseOperationClaim,
    BaseOperationLease, ExternalDeliveryContract, ExternalObservation, ExternalResourceId,
    ImportDecision, InteropDirection, InteropMapping, InteropMappingId, InteropMembrane,
    InteropMembraneLimits, InteropRefusal, OutwardManifestation,
};

pub const ROS_STRING_TYPE: &str = "std_msgs/msg/String";
pub const MAX_ROS_TOPIC_MAPPINGS: u16 = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RosReliability {
    BestEffort,
    Reliable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RosDurability {
    Volatile,
    TransientLocal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RosQos {
    pub reliability: RosReliability,
    pub durability: RosDurability,
    pub history_depth: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RosTopicConfiguration {
    pub mapping: InteropMapping,
    pub topic_name: String,
    pub interface_type: String,
    pub qos: RosQos,
    pub origin_parameter: String,
}

pub struct RosTopicAuthority {
    pub mapping_id: InteropMappingId,
    pub table: BaseCapabilityTable,
    pub handle: BaseCapabilityHandle,
    pub claim: BaseOperationClaim,
}

pub trait NativeRosTopicProvider {
    fn publish(
        &mut self,
        topic_name: &str,
        interface_type: &str,
        qos: RosQos,
        encoded: &[u8],
        origin: &str,
    ) -> Result<(), RosBaseRefusal>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RosBaseRefusal {
    InvalidConfiguration,
    Mapping(InteropRefusal),
    UnknownMapping,
    WrongDirection,
    WrongType,
    PayloadOverflow,
    MalformedString,
    ReflectedManifestation,
    Capability(BaseCapabilityRefusal),
    Provider,
    Inactive,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RosMappingInspection {
    pub mapping_id: InteropMappingId,
    pub semantic_kind: conduit_core::KindId,
    pub direction: InteropDirection,
    pub topic_name: String,
    pub interface_type: String,
    pub qos: RosQos,
    pub maximum_payload_bytes: u32,
    pub maximum_queued_items: u16,
    pub authority_grant_id: conduit_core::AuthorityGrantId,
}

pub struct RosTopicBase {
    membrane: InteropMembrane,
    topics: Vec<RosTopicConfiguration>,
    active: bool,
}

impl RosTopicBase {
    pub fn prepare(topics: Vec<RosTopicConfiguration>) -> Result<Self, RosBaseRefusal> {
        if topics.is_empty() || topics.len() > usize::from(MAX_ROS_TOPIC_MAPPINGS) {
            return Err(RosBaseRefusal::InvalidConfiguration);
        }
        let mut membrane = InteropMembrane::new(InteropMembraneLimits {
            maximum_mappings: MAX_ROS_TOPIC_MAPPINGS,
            maximum_manifestations: MAX_ROS_TOPIC_MAPPINGS,
        })
        .map_err(RosBaseRefusal::Mapping)?;
        for topic in &topics {
            if topic.topic_name.is_empty()
                || topic.interface_type != ROS_STRING_TYPE
                || topic.qos.history_depth == 0
                || topic.qos.history_depth > topic.mapping.maximum_queued_items
                || topic.origin_parameter.is_empty()
            {
                return Err(RosBaseRefusal::InvalidConfiguration);
            }
            membrane
                .register_mapping(topic.mapping.clone())
                .map_err(RosBaseRefusal::Mapping)?;
        }
        Ok(Self {
            membrane,
            topics,
            active: true,
        })
    }

    pub fn inspections(&self) -> impl Iterator<Item = RosMappingInspection> + '_ {
        self.topics.iter().map(|topic| RosMappingInspection {
            mapping_id: topic.mapping.mapping_id.clone(),
            semantic_kind: topic.mapping.semantic_kind.clone(),
            direction: topic.mapping.direction,
            topic_name: topic.topic_name.clone(),
            interface_type: topic.interface_type.clone(),
            qos: topic.qos,
            maximum_payload_bytes: topic.mapping.maximum_payload_bytes,
            maximum_queued_items: topic.mapping.maximum_queued_items,
            authority_grant_id: topic.mapping.authority_grant_id.clone(),
        })
    }

    /// Discovery is diagnostic input only; only configured mappings can admit it.
    pub fn consider_discovery(
        &self,
        topic_name: &str,
        payload_bytes: u32,
        origin: Option<conduit_core::ExternalOrigin>,
    ) -> ImportDecision {
        let Some(topic) = self.topics.iter().find(|topic| {
            topic.mapping.direction == InteropDirection::ExternalToConduit
                && topic.topic_name == topic_name
        }) else {
            return ImportDecision::Unconfigured;
        };
        self.membrane.consider_import(&ExternalObservation {
            adapter_id: topic.mapping.adapter_id.clone(),
            external_resource_id: topic.mapping.external_resource_id.clone(),
            payload_bytes,
            origin,
        })
    }

    pub fn import_string(
        &mut self,
        mapping_id: &InteropMappingId,
        interface_type: &str,
        encoded: &[u8],
        authority: &mut RosTopicAuthority,
    ) -> Result<String, RosBaseRefusal> {
        if !self.active {
            return Err(RosBaseRefusal::Inactive);
        }
        let topic = self.topic(mapping_id, InteropDirection::ExternalToConduit)?;
        validate_frame(topic, interface_type, encoded)?;
        let lease = authorize(authority, mapping_id, encoded.len())?;
        let decoded = decode_ros_string(encoded)?;
        authority
            .table
            .complete(lease, 0)
            .map_err(RosBaseRefusal::Capability)?;
        Ok(decoded)
    }

    pub fn publish_string<P: NativeRosTopicProvider>(
        &mut self,
        mapping_id: &InteropMappingId,
        text: &str,
        authority: &mut RosTopicAuthority,
        provider: &mut P,
    ) -> Result<OutwardManifestation, RosBaseRefusal> {
        if !self.active {
            return Err(RosBaseRefusal::Inactive);
        }
        let topic = self
            .topics
            .iter()
            .find(|topic| {
                &topic.mapping.mapping_id == mapping_id
                    && topic.mapping.direction == InteropDirection::ConduitToExternal
            })
            .ok_or(RosBaseRefusal::UnknownMapping)?;
        let encoded = encode_ros_string(text, topic.mapping.maximum_payload_bytes)?;
        let lease = authorize(authority, mapping_id, encoded.len())?;
        let manifestation = self
            .membrane
            .manifest(mapping_id)
            .map_err(RosBaseRefusal::Mapping)?;
        provider
            .publish(
                &topic.topic_name,
                &topic.interface_type,
                topic.qos,
                &encoded,
                manifestation.origin.manifestation_id.as_str(),
            )
            .map_err(|_| RosBaseRefusal::Provider)?;
        authority
            .table
            .complete(lease, 0)
            .map_err(RosBaseRefusal::Capability)?;
        Ok(manifestation)
    }

    pub fn set_active(&mut self, active: bool) {
        self.active = active;
    }

    fn topic(
        &self,
        mapping_id: &InteropMappingId,
        direction: InteropDirection,
    ) -> Result<&RosTopicConfiguration, RosBaseRefusal> {
        self.topics
            .iter()
            .find(|topic| {
                &topic.mapping.mapping_id == mapping_id && topic.mapping.direction == direction
            })
            .ok_or(RosBaseRefusal::UnknownMapping)
    }
}

fn validate_frame(
    topic: &RosTopicConfiguration,
    interface_type: &str,
    encoded: &[u8],
) -> Result<(), RosBaseRefusal> {
    if interface_type != topic.interface_type {
        return Err(RosBaseRefusal::WrongType);
    }
    if encoded.len() > topic.mapping.maximum_payload_bytes as usize {
        return Err(RosBaseRefusal::PayloadOverflow);
    }
    Ok(())
}

fn authorize(
    authority: &mut RosTopicAuthority,
    mapping_id: &InteropMappingId,
    parameter_bytes: usize,
) -> Result<BaseOperationLease, RosBaseRefusal> {
    if &authority.mapping_id != mapping_id {
        return Err(RosBaseRefusal::WrongDirection);
    }
    authority.claim.parameter_bytes =
        u32::try_from(parameter_bytes).map_err(|_| RosBaseRefusal::PayloadOverflow)?;
    authority
        .table
        .authorize(&authority.handle, &authority.claim)
        .map_err(RosBaseRefusal::Capability)
}

pub fn encode_ros_string(text: &str, maximum: u32) -> Result<Vec<u8>, RosBaseRefusal> {
    let length = u32::try_from(text.len()).map_err(|_| RosBaseRefusal::PayloadOverflow)?;
    let total = length
        .checked_add(4)
        .ok_or(RosBaseRefusal::PayloadOverflow)?;
    if total > maximum {
        return Err(RosBaseRefusal::PayloadOverflow);
    }
    let mut encoded = Vec::with_capacity(total as usize);
    encoded.extend_from_slice(&length.to_le_bytes());
    encoded.extend_from_slice(text.as_bytes());
    Ok(encoded)
}

pub fn decode_ros_string(encoded: &[u8]) -> Result<String, RosBaseRefusal> {
    let length = encoded
        .get(..4)
        .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
        .map(u32::from_le_bytes)
        .and_then(|length| usize::try_from(length).ok())
        .ok_or(RosBaseRefusal::MalformedString)?;
    let bytes = encoded.get(4..).ok_or(RosBaseRefusal::MalformedString)?;
    if bytes.len() != length {
        return Err(RosBaseRefusal::MalformedString);
    }
    String::from_utf8(bytes.to_vec()).map_err(|_| RosBaseRefusal::MalformedString)
}

pub fn topic_resource(topic_name: &str) -> ExternalResourceId {
    ExternalResourceId::from(format!("ros2/topic/{topic_name}"))
}

pub fn qos_delivery(qos: RosQos) -> ExternalDeliveryContract {
    match qos.reliability {
        RosReliability::BestEffort => ExternalDeliveryContract::BestEffort,
        RosReliability::Reliable => ExternalDeliveryContract::AtLeastOnce,
    }
}
