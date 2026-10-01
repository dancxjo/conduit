//! Bounded maker-authored physical environment truth.
//!
//! This document is simulation input. It never becomes a live Host
//! advertisement, Boot observation, capability offer, Base, Sign, or grant.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

mod comparison;

pub use comparison::{EnvironmentComparison, EnvironmentComparisonRow, ObservedPartBinding};

pub const MAKER_ENVIRONMENT_VERSION: u8 = 1;
pub const MAX_AUTHORED_PARTS: usize = 16;
pub const MAX_AUTHORED_LINKS: usize = 32;
pub const MAX_ENVIRONMENT_ID_BYTES: usize = 64;
pub const MAX_PART_NAME_BYTES: usize = 64;
pub const MAX_ENVIRONMENT_COORDINATE: i32 = 32_767;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConnectivityKind {
    Wifi,
    Ethernet,
    Usb,
    UsbCdc,
    Gpio,
    Browser,
    Audio,
    Video,
    Gpu,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MachineProfile {
    PicoW,
    RaspberryPi5,
    LaptopLinux,
}

impl MachineProfile {
    pub fn human_name(self) -> &'static str {
        match self {
            Self::PicoW => "Pico W",
            Self::RaspberryPi5 => "Raspberry Pi 5",
            Self::LaptopLinux => "Laptop-class Linux host",
        }
    }

    pub fn expected_observed_profile(self) -> &'static str {
        match self {
            Self::PicoW => "conduit-pico-w",
            Self::RaspberryPi5 => "conduit-linux-rpi5",
            Self::LaptopLinux => "conduit-linux-laptop",
        }
    }

    pub fn reviewed_resources(self) -> PartResources {
        let (compute_units, memory_bytes, connectivity) = match self {
            Self::PicoW => (
                1,
                264 * 1024,
                vec![
                    ConnectivityKind::Wifi,
                    ConnectivityKind::UsbCdc,
                    ConnectivityKind::Gpio,
                ],
            ),
            Self::RaspberryPi5 => (
                8,
                8 * 1024 * 1024 * 1024,
                vec![
                    ConnectivityKind::Wifi,
                    ConnectivityKind::Ethernet,
                    ConnectivityKind::Usb,
                    ConnectivityKind::Audio,
                    ConnectivityKind::Video,
                ],
            ),
            Self::LaptopLinux => (
                16,
                16 * 1024 * 1024 * 1024,
                vec![
                    ConnectivityKind::Wifi,
                    ConnectivityKind::Ethernet,
                    ConnectivityKind::Usb,
                    ConnectivityKind::Browser,
                    ConnectivityKind::Audio,
                    ConnectivityKind::Video,
                    ConnectivityKind::Gpu,
                ],
            ),
        };
        PartResources {
            compute_units,
            memory_bytes,
            connectivity,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PartResources {
    pub compute_units: u16,
    pub memory_bytes: u64,
    pub connectivity: Vec<ConnectivityKind>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoredPart {
    pub part_id: String,
    pub name: String,
    pub profile: MachineProfile,
    pub resources: PartResources,
    pub x: i32,
    pub y: i32,
}

impl AuthoredPart {
    pub fn reviewed(
        part_id: impl Into<String>,
        name: impl Into<String>,
        profile: MachineProfile,
    ) -> Self {
        Self {
            part_id: part_id.into(),
            name: name.into(),
            profile,
            resources: profile.reviewed_resources(),
            x: 0,
            y: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EnvironmentLinkKind {
    Wifi,
    Ethernet,
    Usb,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoredLink {
    pub link_id: String,
    pub left_part_id: String,
    pub right_part_id: String,
    pub kind: EnvironmentLinkKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoredEnvironment {
    pub version: u8,
    pub environment_id: String,
    pub revision: u64,
    pub parts: Vec<AuthoredPart>,
    pub links: Vec<AuthoredLink>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthoredEnvironmentError {
    WrongVersion,
    InvalidEnvironmentId,
    InvalidPartId,
    InvalidPartName,
    CoordinateOutOfBounds,
    InvalidResources,
    DuplicatePart,
    TooManyParts,
    InvalidLinkId,
    DuplicateLink,
    UnknownLinkPart,
    UnsupportedLink,
    TooManyLinks,
    UnknownPart,
    InvalidObservation(String),
    DuplicateBinding,
    UnknownBindingPart,
}

impl std::fmt::Display for AuthoredEnvironmentError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for AuthoredEnvironmentError {}

impl AuthoredEnvironment {
    pub fn new(environment_id: impl Into<String>) -> Result<Self, AuthoredEnvironmentError> {
        let document = Self {
            version: MAKER_ENVIRONMENT_VERSION,
            environment_id: environment_id.into(),
            revision: 1,
            parts: Vec::new(),
            links: Vec::new(),
        };
        document.validate()?;
        Ok(document)
    }

    pub fn validate(&self) -> Result<(), AuthoredEnvironmentError> {
        if self.version != MAKER_ENVIRONMENT_VERSION {
            return Err(AuthoredEnvironmentError::WrongVersion);
        }
        validate_identity(&self.environment_id, MAX_ENVIRONMENT_ID_BYTES)
            .map_err(|_| AuthoredEnvironmentError::InvalidEnvironmentId)?;
        if self.revision == 0 || self.parts.len() > MAX_AUTHORED_PARTS {
            return Err(if self.revision == 0 {
                AuthoredEnvironmentError::WrongVersion
            } else {
                AuthoredEnvironmentError::TooManyParts
            });
        }
        if self.links.len() > MAX_AUTHORED_LINKS {
            return Err(AuthoredEnvironmentError::TooManyLinks);
        }
        let mut part_ids = BTreeSet::new();
        for part in &self.parts {
            validate_identity(&part.part_id, MAX_ENVIRONMENT_ID_BYTES)
                .map_err(|_| AuthoredEnvironmentError::InvalidPartId)?;
            if !part_ids.insert(part.part_id.as_str()) {
                return Err(AuthoredEnvironmentError::DuplicatePart);
            }
            if part.name.is_empty() || part.name.len() > MAX_PART_NAME_BYTES {
                return Err(AuthoredEnvironmentError::InvalidPartName);
            }
            validate_coordinate(part.x, part.y)?;
            validate_resources(part)?;
        }
        let mut link_ids = BTreeSet::new();
        let mut link_pairs = BTreeSet::new();
        for link in &self.links {
            validate_identity(&link.link_id, MAX_ENVIRONMENT_ID_BYTES)
                .map_err(|_| AuthoredEnvironmentError::InvalidLinkId)?;
            if !link_ids.insert(link.link_id.as_str()) {
                return Err(AuthoredEnvironmentError::DuplicateLink);
            }
            if link.left_part_id == link.right_part_id
                || !part_ids.contains(link.left_part_id.as_str())
                || !part_ids.contains(link.right_part_id.as_str())
            {
                return Err(AuthoredEnvironmentError::UnknownLinkPart);
            }
            let pair = if link.left_part_id < link.right_part_id {
                (&link.left_part_id, &link.right_part_id, link.kind)
            } else {
                (&link.right_part_id, &link.left_part_id, link.kind)
            };
            if !link_pairs.insert(pair) {
                return Err(AuthoredEnvironmentError::DuplicateLink);
            }
            validate_link_support(self, link)?;
        }
        Ok(())
    }

    pub fn add_part(&mut self, part: AuthoredPart) -> Result<(), AuthoredEnvironmentError> {
        if self.parts.len() == MAX_AUTHORED_PARTS {
            return Err(AuthoredEnvironmentError::TooManyParts);
        }
        self.parts.push(part);
        if let Err(error) = self.validate() {
            self.parts.pop();
            return Err(error);
        }
        self.bump_revision();
        Ok(())
    }

    pub fn rename_part(
        &mut self,
        part_id: &str,
        name: String,
    ) -> Result<(), AuthoredEnvironmentError> {
        if name.is_empty() || name.len() > MAX_PART_NAME_BYTES {
            return Err(AuthoredEnvironmentError::InvalidPartName);
        }
        let part = self.part_mut(part_id)?;
        part.name = name;
        self.bump_revision();
        Ok(())
    }

    pub fn move_part(
        &mut self,
        part_id: &str,
        x: i32,
        y: i32,
    ) -> Result<(), AuthoredEnvironmentError> {
        validate_coordinate(x, y)?;
        let part = self.part_mut(part_id)?;
        part.x = x;
        part.y = y;
        self.bump_revision();
        Ok(())
    }

    pub fn remove_part(&mut self, part_id: &str) -> Result<(), AuthoredEnvironmentError> {
        let index = self
            .parts
            .iter()
            .position(|part| part.part_id == part_id)
            .ok_or(AuthoredEnvironmentError::UnknownPart)?;
        self.parts.remove(index);
        self.links
            .retain(|link| link.left_part_id != part_id && link.right_part_id != part_id);
        self.bump_revision();
        Ok(())
    }

    pub fn add_link(&mut self, link: AuthoredLink) -> Result<(), AuthoredEnvironmentError> {
        if self.links.len() == MAX_AUTHORED_LINKS {
            return Err(AuthoredEnvironmentError::TooManyLinks);
        }
        self.links.push(link);
        if let Err(error) = self.validate() {
            self.links.pop();
            return Err(error);
        }
        self.bump_revision();
        Ok(())
    }

    pub fn simulation_projection(&self) -> Result<SimulationProjection, AuthoredEnvironmentError> {
        self.validate()?;
        Ok(SimulationProjection {
            environment_id: self.environment_id.clone(),
            environment_revision: self.revision,
            provenance: SimulationProvenance {
                proof_class: "authored-environment-simulation",
                observed_live_truth: false,
                physical_evidence: false,
                authority_granted: false,
            },
            hosts: self
                .parts
                .iter()
                .map(|part| SimulationHostCandidate {
                    part_id: part.part_id.clone(),
                    host_id: format!(
                        "simulation/environment/{}/part/{}",
                        self.environment_id, part.part_id
                    ),
                    boot_id: format!(
                        "simulation/environment/{}/revision/{}/part/{}",
                        self.environment_id, self.revision, part.part_id
                    ),
                    profile: part.profile,
                    resources: part.resources.clone(),
                })
                .collect(),
        })
    }

    fn part_mut(&mut self, part_id: &str) -> Result<&mut AuthoredPart, AuthoredEnvironmentError> {
        self.parts
            .iter_mut()
            .find(|part| part.part_id == part_id)
            .ok_or(AuthoredEnvironmentError::UnknownPart)
    }

    fn bump_revision(&mut self) {
        self.revision = self.revision.saturating_add(1);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SimulationProvenance {
    pub proof_class: &'static str,
    pub observed_live_truth: bool,
    pub physical_evidence: bool,
    pub authority_granted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulationHostCandidate {
    pub part_id: String,
    pub host_id: String,
    pub boot_id: String,
    pub profile: MachineProfile,
    pub resources: PartResources,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulationProjection {
    pub environment_id: String,
    pub environment_revision: u64,
    pub provenance: SimulationProvenance,
    pub hosts: Vec<SimulationHostCandidate>,
}

fn validate_identity(value: &str, maximum: usize) -> Result<(), ()> {
    if !value.is_empty()
        && value.len() <= maximum
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        Ok(())
    } else {
        Err(())
    }
}

fn validate_coordinate(x: i32, y: i32) -> Result<(), AuthoredEnvironmentError> {
    if x.unsigned_abs() <= MAX_ENVIRONMENT_COORDINATE as u32
        && y.unsigned_abs() <= MAX_ENVIRONMENT_COORDINATE as u32
    {
        Ok(())
    } else {
        Err(AuthoredEnvironmentError::CoordinateOutOfBounds)
    }
}

fn validate_resources(part: &AuthoredPart) -> Result<(), AuthoredEnvironmentError> {
    let reviewed = part.profile.reviewed_resources();
    let unique = part
        .resources
        .connectivity
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if part.resources.compute_units == 0
        || part.resources.compute_units > reviewed.compute_units
        || part.resources.memory_bytes == 0
        || part.resources.memory_bytes > reviewed.memory_bytes
        || unique.len() != part.resources.connectivity.len()
        || !unique
            .iter()
            .all(|kind| reviewed.connectivity.contains(kind))
    {
        Err(AuthoredEnvironmentError::InvalidResources)
    } else {
        Ok(())
    }
}

fn validate_link_support(
    environment: &AuthoredEnvironment,
    link: &AuthoredLink,
) -> Result<(), AuthoredEnvironmentError> {
    let required = match link.kind {
        EnvironmentLinkKind::Wifi => ConnectivityKind::Wifi,
        EnvironmentLinkKind::Ethernet => ConnectivityKind::Ethernet,
        EnvironmentLinkKind::Usb => ConnectivityKind::Usb,
    };
    let supports = |part_id: &str| {
        environment
            .parts
            .iter()
            .find(|part| part.part_id == part_id)
            .is_some_and(|part| {
                part.resources.connectivity.contains(&required)
                    || (link.kind == EnvironmentLinkKind::Usb
                        && part
                            .resources
                            .connectivity
                            .contains(&ConnectivityKind::UsbCdc))
            })
    };
    if supports(&link.left_part_id) && supports(&link.right_part_id) {
        Ok(())
    } else {
        Err(AuthoredEnvironmentError::UnsupportedLink)
    }
}
