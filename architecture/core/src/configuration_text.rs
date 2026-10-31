//! Immutable configuration text can retain linked Make material without copying it.
//! Storage is absent from serialization, equality, ordering and fingerprints.
use alloc::{borrow::Cow, string::String, vec::Vec};
use core::{
    cmp::Ordering,
    fmt,
    hash::{Hash, Hasher},
    ops::Deref,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone)]
pub struct ConfigurationText(Cow<'static, str>);
impl ConfigurationText {
    /// Borrow immutable text linked into this executable. This conveys no
    /// checked Source provenance, selected implementation or execution authority.
    pub const fn from_static(text: &'static str) -> Self {
        Self(Cow::Borrowed(text))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn into_bytes(self) -> Vec<u8> {
        self.0.into_owned().into_bytes()
    }
    pub fn into_string(self) -> String {
        self.0.into_owned()
    }
}
impl From<String> for ConfigurationText {
    fn from(value: String) -> Self {
        Self(Cow::Owned(value))
    }
}
impl From<&str> for ConfigurationText {
    fn from(value: &str) -> Self {
        Self(Cow::Owned(value.into()))
    }
}
impl Deref for ConfigurationText {
    type Target = str;
    fn deref(&self) -> &str {
        self.as_str()
    }
}
impl AsRef<str> for ConfigurationText {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}
impl<T: AsRef<str> + ?Sized> PartialEq<T> for ConfigurationText {
    fn eq(&self, other: &T) -> bool {
        self.as_str() == other.as_ref()
    }
}
impl Eq for ConfigurationText {}
impl<T: AsRef<str> + ?Sized> PartialOrd<T> for ConfigurationText {
    fn partial_cmp(&self, other: &T) -> Option<Ordering> {
        Some(self.as_str().cmp(other.as_ref()))
    }
}
impl Ord for ConfigurationText {
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_str().cmp(other.as_str())
    }
}
impl Hash for ConfigurationText {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_str().hash(state)
    }
}
impl fmt::Debug for ConfigurationText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), f)
    }
}
impl fmt::Display for ConfigurationText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
impl Serialize for ConfigurationText {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}
impl<'de> Deserialize<'de> for ConfigurationText {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        String::deserialize(d).map(Self::from)
    }
}
impl AsRef<[u8]> for ConfigurationText {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl PartialEq<ConfigurationText> for str {
    fn eq(&self, other: &ConfigurationText) -> bool {
        self == other.as_str()
    }
}
impl PartialEq<ConfigurationText> for String {
    fn eq(&self, other: &ConfigurationText) -> bool {
        self.as_str() == other.as_str()
    }
}
impl From<ConfigurationText> for String {
    fn from(text: ConfigurationText) -> Self {
        text.into_string()
    }
}
impl Default for ConfigurationText {
    fn default() -> Self {
        Self::from_static("")
    }
}
