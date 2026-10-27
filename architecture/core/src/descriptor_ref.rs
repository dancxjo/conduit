//! Complete immutable descriptor views for linked and owned contracts.
//! These descriptions confer no Source verification, selection or possession.
use crate::{HostCallRequirement, PortDescriptor, PortDirection, PortTemporal};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PortDescriptorRef<'a> {
    pub port_id: &'a str,
    pub value_kind: &'a str,
    pub direction: PortDirection,
    pub temporal: PortTemporal,
    pub abnormal_kind: Option<&'a str>,
}
impl<'a> From<&'a PortDescriptor> for PortDescriptorRef<'a> {
    fn from(port: &'a PortDescriptor) -> Self {
        Self {
            port_id: port.port_id.as_str(),
            value_kind: port.value_kind.as_str(),
            direction: port.direction,
            temporal: port.temporal,
            abnormal_kind: port.abnormal_kind.as_ref().map(|id| id.as_str()),
        }
    }
}
impl From<PortDescriptorRef<'_>> for PortDescriptor {
    fn from(port: PortDescriptorRef<'_>) -> Self {
        Self {
            port_id: port.port_id.into(),
            value_kind: port.value_kind.into(),
            direction: port.direction,
            temporal: port.temporal,
            abnormal_kind: port.abnormal_kind.map(Into::into),
        }
    }
}
#[derive(Clone, Copy)]
pub enum PortDescriptorRefs<'a> {
    Owned(&'a [PortDescriptor]),
    Borrowed(&'a [PortDescriptorRef<'a>]),
}
impl<'a> PortDescriptorRefs<'a> {
    pub fn len(self) -> usize {
        match self {
            Self::Owned(v) => v.len(),
            Self::Borrowed(v) => v.len(),
        }
    }
    pub fn is_empty(self) -> bool {
        self.len() == 0
    }
    pub fn get(self, index: usize) -> Option<PortDescriptorRef<'a>> {
        match self {
            Self::Owned(v) => v.get(index).map(Into::into),
            Self::Borrowed(v) => v.get(index).copied(),
        }
    }
    /// Equality after the same stable port-id ordering as `CheckedFront`.
    /// Equal-key occurrences retain their original order; no descriptor is
    /// deduplicated or omitted and no temporary sorted allocation is needed.
    pub fn canonical_eq(self, other: PortDescriptorRefs<'_>) -> bool {
        self.len() == other.len()
            && self.iter().enumerate().all(|(index, port)| {
                let occurrence = self
                    .iter()
                    .take(index)
                    .filter(|previous| previous.port_id == port.port_id)
                    .count();
                other
                    .iter()
                    .filter(|candidate| candidate.port_id == port.port_id)
                    .nth(occurrence)
                    == Some(port)
            })
    }
    pub fn iter(self) -> impl ExactSizeIterator<Item = PortDescriptorRef<'a>> {
        (0..self.len()).map(move |i| match self {
            Self::Owned(v) => (&v[i]).into(),
            Self::Borrowed(v) => v[i],
        })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct HostCallRequirementRef<'a> {
    pub contract_id: &'a str,
    pub target_kind: Option<&'a str>,
    pub maximum_in_flight: u16,
    pub maximum_input_bytes: u32,
    pub maximum_output_bytes: u32,
}
impl<'a> From<&'a HostCallRequirement> for HostCallRequirementRef<'a> {
    fn from(call: &'a HostCallRequirement) -> Self {
        Self {
            contract_id: call.contract_id.as_str(),
            target_kind: call.target_kind.as_ref().map(|id| id.as_str()),
            maximum_in_flight: call.maximum_in_flight,
            maximum_input_bytes: call.maximum_input_bytes,
            maximum_output_bytes: call.maximum_output_bytes,
        }
    }
}
#[derive(Clone, Copy)]
pub enum HostCallRequirementRefs<'a> {
    Owned(&'a [HostCallRequirement]),
    Borrowed(&'a [HostCallRequirementRef<'a>]),
}
impl<'a> HostCallRequirementRefs<'a> {
    pub fn len(self) -> usize {
        match self {
            Self::Owned(v) => v.len(),
            Self::Borrowed(v) => v.len(),
        }
    }
    pub fn is_empty(self) -> bool {
        self.len() == 0
    }
    pub fn iter(self) -> impl ExactSizeIterator<Item = HostCallRequirementRef<'a>> {
        (0..self.len()).map(move |i| match self {
            Self::Owned(v) => (&v[i]).into(),
            Self::Borrowed(v) => v[i],
        })
    }
}
