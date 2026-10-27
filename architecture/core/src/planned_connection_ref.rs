//! Borrowed fingerprint records. None establish Plan validity or resource possession.
use crate::*;
#[derive(Clone, Copy)]
pub struct PlannedResourceConnectionRef<'a> {
    pub port_id: &'a str,
    pub class_id: &'a str,
    pub ownership: ResourcePortOwnership,
    pub lifecycle: ResourcePortLifecycle,
    pub mobility: ResourcePortMobility,
    pub owner_placement_id: &'a str,
    pub source_binding: ResourceBindingRef<'a>,
}
impl<'a> From<&'a PlannedResourceConnection> for PlannedResourceConnectionRef<'a> {
    fn from(v: &'a PlannedResourceConnection) -> Self {
        Self {
            port_id: v.contract.port_id.as_str(),
            class_id: v.contract.class_id.as_str(),
            ownership: v.contract.ownership,
            lifecycle: v.contract.lifecycle,
            mobility: v.contract.mobility,
            owner_placement_id: v.owner_placement_id.as_str(),
            source_binding: (&v.source_binding).into(),
        }
    }
}
#[derive(Clone, Copy)]
pub struct PlannedConnectionRef<'a> {
    pub connection_id: &'a str,
    pub source_placement_id: &'a str,
    pub source_port_id: &'a str,
    pub sink_placement_id: &'a str,
    pub sink_port_id: &'a str,
    pub value_kind: &'a str,
    pub resource: Option<PlannedResourceConnectionRef<'a>>,
    pub abnormal_kind: Option<&'a str>,
    pub track: ConnectionTrack,
    pub temporal: PortTemporal,
    pub pressure_policy: DeliveryPressurePolicy,
    pub selected_line: Option<&'a AdmittedLine>,
    pub admitted_lines: &'a [AdmittedLine],
    pub item_capacity: u16,
    pub byte_capacity: u32,
}
impl<'a> From<&'a PlannedConnection> for PlannedConnectionRef<'a> {
    fn from(v: &'a PlannedConnection) -> Self {
        Self {
            connection_id: v.connection_id.as_str(),
            source_placement_id: v.source_placement_id.as_str(),
            source_port_id: v.source_port_id.as_str(),
            sink_placement_id: v.sink_placement_id.as_str(),
            sink_port_id: v.sink_port_id.as_str(),
            value_kind: v.value_kind.as_str(),
            resource: v.resource.as_deref().map(Into::into),
            abnormal_kind: v.abnormal_kind.as_ref().map(|v| v.as_str()),
            track: v.track,
            temporal: v.temporal,
            pressure_policy: v.pressure_policy,
            selected_line: v.selected_line.as_deref(),
            admitted_lines: &v.admitted_lines,
            item_capacity: v.item_capacity,
            byte_capacity: v.byte_capacity,
        }
    }
}
#[derive(Clone, Copy)]
pub struct StartupDependencyRef<'a> {
    pub prerequisite_placement_id: &'a str,
    pub dependent_placement_id: &'a str,
}
impl<'a> From<&'a StartupDependency> for StartupDependencyRef<'a> {
    fn from(v: &'a StartupDependency) -> Self {
        Self {
            prerequisite_placement_id: v.prerequisite_placement_id.as_str(),
            dependent_placement_id: v.dependent_placement_id.as_str(),
        }
    }
}
#[derive(Clone, Copy)]
pub enum ExpectedTerminalRef<'a> {
    PlacementCompleted(&'a str),
    ConnectionCompleted(&'a str),
    PlanCompleted,
}
impl<'a> From<&'a ExpectedTerminal> for ExpectedTerminalRef<'a> {
    fn from(v: &'a ExpectedTerminal) -> Self {
        match v {
            ExpectedTerminal::PlacementCompleted(v) => Self::PlacementCompleted(v.as_str()),
            ExpectedTerminal::ConnectionCompleted(v) => Self::ConnectionCompleted(v.as_str()),
            ExpectedTerminal::PlanCompleted => Self::PlanCompleted,
        }
    }
}
#[derive(Clone, Copy)]
pub enum ExpectedSignRef<'a> {
    PlanFragmentReceived,
    PlacementPrepared(&'a str),
    PlacementTerminal(&'a str),
    ConnectionTerminal(&'a str),
    PlanTerminal,
}
impl<'a> From<&'a ExpectedSign> for ExpectedSignRef<'a> {
    fn from(v: &'a ExpectedSign) -> Self {
        match v {
            ExpectedSign::PlanFragmentReceived => Self::PlanFragmentReceived,
            ExpectedSign::PlacementPrepared(v) => Self::PlacementPrepared(v.as_str()),
            ExpectedSign::PlacementTerminal(v) => Self::PlacementTerminal(v.as_str()),
            ExpectedSign::ConnectionTerminal(v) => Self::ConnectionTerminal(v.as_str()),
            ExpectedSign::PlanTerminal => Self::PlanTerminal,
        }
    }
}
/// Existing owned records or independently assembled immutable views.
#[derive(Clone, Copy)]
pub enum PlannedConnectionRefs<'a> {
    Owned(&'a [PlannedConnection]),
    Borrowed(&'a [PlannedConnectionRef<'a>]),
}
impl<'a> PlannedConnectionRefs<'a> {
    pub fn len(self) -> usize {
        match self {
            Self::Owned(v) => v.len(),
            Self::Borrowed(v) => v.len(),
        }
    }
    pub fn is_empty(self) -> bool {
        self.len() == 0
    }
    pub fn iter(self) -> impl ExactSizeIterator<Item = PlannedConnectionRef<'a>> {
        (0..self.len()).map(move |i| match self {
            Self::Owned(v) => (&v[i]).into(),
            Self::Borrowed(v) => v[i],
        })
    }
}
/// Existing owned records or independently assembled immutable views.
#[derive(Clone, Copy)]
pub enum StartupDependencyRefs<'a> {
    Owned(&'a [StartupDependency]),
    Borrowed(&'a [StartupDependencyRef<'a>]),
}
impl<'a> StartupDependencyRefs<'a> {
    pub fn len(self) -> usize {
        match self {
            Self::Owned(v) => v.len(),
            Self::Borrowed(v) => v.len(),
        }
    }
    pub fn is_empty(self) -> bool {
        self.len() == 0
    }
    pub fn iter(self) -> impl ExactSizeIterator<Item = StartupDependencyRef<'a>> {
        (0..self.len()).map(move |i| match self {
            Self::Owned(v) => (&v[i]).into(),
            Self::Borrowed(v) => v[i],
        })
    }
}
/// Existing owned records or independently assembled immutable views.
#[derive(Clone, Copy)]
pub enum ExpectedTerminalRefs<'a> {
    Owned(&'a [ExpectedTerminal]),
    Borrowed(&'a [ExpectedTerminalRef<'a>]),
}
impl<'a> ExpectedTerminalRefs<'a> {
    pub fn len(self) -> usize {
        match self {
            Self::Owned(v) => v.len(),
            Self::Borrowed(v) => v.len(),
        }
    }
    pub fn is_empty(self) -> bool {
        self.len() == 0
    }
    pub fn iter(self) -> impl ExactSizeIterator<Item = ExpectedTerminalRef<'a>> {
        (0..self.len()).map(move |i| match self {
            Self::Owned(v) => (&v[i]).into(),
            Self::Borrowed(v) => v[i],
        })
    }
}
/// Existing owned records or independently assembled immutable views.
#[derive(Clone, Copy)]
pub enum ExpectedSignRefs<'a> {
    Owned(&'a [ExpectedSign]),
    Borrowed(&'a [ExpectedSignRef<'a>]),
}
impl<'a> ExpectedSignRefs<'a> {
    pub fn len(self) -> usize {
        match self {
            Self::Owned(v) => v.len(),
            Self::Borrowed(v) => v.len(),
        }
    }
    pub fn is_empty(self) -> bool {
        self.len() == 0
    }
    pub fn iter(self) -> impl ExactSizeIterator<Item = ExpectedSignRef<'a>> {
        (0..self.len()).map(move |i| match self {
            Self::Owned(v) => (&v[i]).into(),
            Self::Borrowed(v) => v[i],
        })
    }
}
/// Borrowed placement identities in the original canonical order.
#[derive(Clone, Copy)]
pub enum PlacementIdRefs<'a> {
    Owned(&'a [PlacementId]),
    Borrowed(&'a [&'a str]),
}
impl<'a> PlacementIdRefs<'a> {
    pub fn len(self) -> usize {
        match self {
            Self::Owned(v) => v.len(),
            Self::Borrowed(v) => v.len(),
        }
    }
    pub fn is_empty(self) -> bool {
        self.len() == 0
    }
    pub fn iter(self) -> impl ExactSizeIterator<Item = &'a str> {
        (0..self.len()).map(move |i| match self {
            Self::Owned(v) => v[i].as_str(),
            Self::Borrowed(v) => v[i],
        })
    }
}
