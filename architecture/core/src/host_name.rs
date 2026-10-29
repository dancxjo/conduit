//! Bounded declared Host names. Names never replace cryptographic identities.
use alloc::string::String;
use serde::{Deserialize, Serialize};

pub const MAX_HOST_DECLARED_NAME_BYTES: usize = 160;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostNameRefusal {
    Empty,
    TooLong,
    ControlCharacter,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct HostDeclaredName(String);
impl HostDeclaredName {
    pub fn new(name: impl Into<String>) -> Result<Self, HostNameRefusal> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(HostNameRefusal::Empty);
        }
        if name.len() > MAX_HOST_DECLARED_NAME_BYTES {
            return Err(HostNameRefusal::TooLong);
        }
        if name.chars().any(char::is_control) {
            return Err(HostNameRefusal::ControlCharacter);
        }
        Ok(Self(name))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for HostDeclaredName {
    type Error = HostNameRefusal;
    fn try_from(name: String) -> Result<Self, Self::Error> {
        Self::new(name)
    }
}
impl From<HostDeclaredName> for String {
    fn from(name: HostDeclaredName) -> Self {
        name.0
    }
}
impl core::fmt::Display for HostNameRefusal {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "invalid declared Host name: {self:?}")
    }
}
impl crate::HostAdvertisement {
    /// Wording only. Equal names do not imply equal Hosts or current membership.
    pub fn display_name(&self) -> &str {
        self.declared_name
            .as_ref()
            .map_or(self.host_id.as_str(), HostDeclaredName::as_str)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn declared_names_are_bounded_at_construction_and_decode() {
        for value in ["", "  ", "yellow\n", &"x".repeat(161)] {
            assert!(HostDeclaredName::new(value).is_err());
            assert!(serde_json::from_value::<HostDeclaredName>(serde_json::json!(value)).is_err());
        }
        let name = HostDeclaredName::new("Yellow").unwrap();
        assert_eq!(serde_json::to_string(&name).unwrap(), "\"Yellow\"");
        assert_eq!(
            serde_json::from_str::<HostDeclaredName>("\"Yellow\"").unwrap(),
            name
        );
    }
    #[test]
    fn unnamed_wire_roundtrip_remains_unchanged_and_name_is_not_identity() {
        let wire = serde_json::json!({"protocol_version":crate::PROTOCOL_VERSION,"host_id":"host/one","boot_id":"boot/one","offer_generation":1,"profile":"profile/test","resources":[],"capabilities":[],"planner_capabilities":[]});
        let mut host: crate::HostAdvertisement = serde_json::from_value(wire).unwrap();
        let original = serde_json::to_value(&host).unwrap();
        assert!(original.get("declared_name").is_none());
        assert_eq!(host.display_name(), "host/one");
        let identity = (host.host_id.clone(), host.boot_id.clone());
        host.declared_name = Some(HostDeclaredName::new("Yellow").unwrap());
        assert_eq!(host.display_name(), "Yellow");
        assert_eq!((host.host_id.clone(), host.boot_id.clone()), identity);
        let decoded: crate::HostAdvertisement =
            serde_json::from_value(serde_json::to_value(&host).unwrap()).unwrap();
        assert_eq!(decoded, host);
        host.declared_name = None;
        assert_eq!(serde_json::to_value(host).unwrap(), original);
    }
}
