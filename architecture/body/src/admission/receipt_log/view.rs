use super::*;
#[derive(Clone, Copy, Debug)]
pub enum ReceiptIdentityRef<'a> {
    Text(&'a str),
    Hex(&'a [u8; 32]),
}
pub(super) fn packed(text: &str) -> Option<[u8; 32]> {
    if text.len() != 64 {
        return None;
    }
    let digit = |b| match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        _ => None,
    };
    let mut out = [0; 32];
    for (pair, byte) in text.as_bytes().chunks_exact(2).zip(out.iter_mut()) {
        *byte = digit(pair[0])? * 16 + digit(pair[1])?;
    }
    Some(out)
}
impl ReceiptIdentityRef<'_> {
    pub fn equals(self, text: &str) -> bool {
        match self {
            Self::Text(value) => value == text,
            Self::Hex(value) => packed(text).as_ref() == Some(value),
        }
    }
}
impl PartialEq for ReceiptIdentityRef<'_> {
    fn eq(&self, other: &Self) -> bool {
        match (*self, *other) {
            (Self::Text(a), Self::Text(b)) => a == b,
            (Self::Hex(a), Self::Hex(b)) => a == b,
            (a, Self::Text(b)) => a.equals(b),
            (Self::Text(a), b) => b.equals(a),
        }
    }
}
impl Eq for ReceiptIdentityRef<'_> {}
impl Serialize for ReceiptIdentityRef<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Text(text) => serializer.serialize_str(text),
            Self::Hex(bytes) => {
                let mut text = [0; 64];
                let digits = b"0123456789abcdef";
                for (pair, byte) in text.chunks_exact_mut(2).zip(bytes.iter()) {
                    pair[0] = digits[usize::from(byte >> 4)];
                    pair[1] = digits[usize::from(byte & 15)];
                }
                serializer.serialize_str(core::str::from_utf8(&text).expect("hexadecimal"))
            }
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename = "MembershipCredential")]
pub struct ReceiptCredentialRef<'a> {
    #[serde(serialize_with = "credential_id_serde")]
    pub credential_id: ReceiptIdentityRef<'a>,
    #[serde(serialize_with = "body_id_serde")]
    pub body_id: &'a str,
    #[serde(serialize_with = "part_id_serde")]
    pub part_id: &'a str,
    #[serde(serialize_with = "host_id_serde")]
    pub host_id: &'a str,
    #[serde(serialize_with = "boot_id_serde")]
    pub boot_id: &'a str,
    pub issued_at_millis: u64,
}
impl ReceiptCredentialRef<'_> {
    /// Compare exact historical identity without reconstructing an owned row.
    pub fn matches(&self, credential: &MembershipCredential) -> bool {
        self.credential_id.equals(credential.credential_id.as_str())
            && self.body_id == credential.body_id.as_str()
            && self.part_id == credential.part_id.as_str()
            && self.host_id == credential.host_id.as_str()
            && self.boot_id == credential.boot_id.as_str()
            && self.issued_at_millis == credential.issued_at_millis
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename = "AdmissionReceipt")]
pub struct AdmissionReceiptRef<'a> {
    #[serde(serialize_with = "admission_id_serde")]
    pub admission_id: ReceiptIdentityRef<'a>,
    pub credential: ReceiptCredentialRef<'a>,
}
impl<'a> From<&'a AdmissionReceipt> for AdmissionReceiptRef<'a> {
    fn from(receipt: &'a AdmissionReceipt) -> Self {
        let c = &receipt.credential;
        Self {
            admission_id: ReceiptIdentityRef::Text(receipt.admission_id.as_str()),
            credential: ReceiptCredentialRef {
                credential_id: ReceiptIdentityRef::Text(c.credential_id.as_str()),
                body_id: c.body_id.as_str(),
                part_id: c.part_id.as_str(),
                host_id: c.host_id.as_str(),
                boot_id: c.boot_id.as_str(),
                issued_at_millis: c.issued_at_millis,
            },
        }
    }
}
impl<'a> AdmissionReceiptRef<'a> {
    pub(super) fn texts(&self) -> [&'a str; 4] {
        let c = self.credential;
        [c.body_id, c.part_id, c.host_id, c.boot_id]
    }
    pub(super) fn digests(&self) -> Option<([u8; 32], [u8; 32])> {
        let extract = |id| match id {
            ReceiptIdentityRef::Hex(v) => Some(*v),
            ReceiptIdentityRef::Text(v) => packed(v),
        };
        Some((
            extract(self.admission_id)?,
            extract(self.credential.credential_id)?,
        ))
    }
}

// Preserve the original identity newtype model for non-JSON serializers too.
macro_rules! identity_serde {
    ($function:ident, $name:literal, $ty:ty) => {
        fn $function<S: serde::Serializer>(value: &$ty, serializer: S) -> Result<S::Ok, S::Error> {
            serializer.serialize_newtype_struct($name, value)
        }
    };
}
identity_serde!(
    credential_id_serde,
    "MembershipCredentialId",
    ReceiptIdentityRef<'_>
);
identity_serde!(admission_id_serde, "AdmissionId", ReceiptIdentityRef<'_>);
identity_serde!(body_id_serde, "BodyId", &str);
identity_serde!(part_id_serde, "PartId", &str);
identity_serde!(host_id_serde, "HostId", &str);
identity_serde!(boot_id_serde, "BootId", &str);

#[cfg(test)]
mod matching_tests {
    use super::*;
    #[test]
    fn exact_comparison_checks_every_credential_field_and_packed_identity() {
        let credential = MembershipCredential {
            credential_id: "ab".repeat(32).into(),
            body_id: "body/one".into(),
            part_id: "part/one".into(),
            host_id: "host/one".into(),
            boot_id: "boot/one".into(),
            issued_at_millis: 7,
        };
        let packed_id = [0xab; 32];
        let view = ReceiptCredentialRef {
            credential_id: ReceiptIdentityRef::Hex(&packed_id),
            body_id: "body/one",
            part_id: "part/one",
            host_id: "host/one",
            boot_id: "boot/one",
            issued_at_millis: 7,
        };
        assert!(view.matches(&credential));
        for field in 0..6 {
            let mut changed = credential.clone();
            match field {
                0 => changed.credential_id = "cd".repeat(32).into(),
                1 => changed.body_id = "body/other".into(),
                2 => changed.part_id = "part/other".into(),
                3 => changed.host_id = "host/other".into(),
                4 => changed.boot_id = "boot/other".into(),
                _ => changed.issued_at_millis += 1,
            }
            assert!(!view.matches(&changed));
        }
    }
}
