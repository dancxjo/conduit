//! Stable bounded identities shared by Root effect scope admission.
use crate::protected_region::DomainRefusal;
use sha2::{Digest, Sha256};

pub(crate) fn identity(domain: &[u8], fields: &[&[u8]]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"conduit.conduitos/domain-effect-scope@1");
    digest.update((domain.len() as u32).to_le_bytes());
    digest.update(domain);
    for field in fields {
        digest.update((field.len() as u32).to_le_bytes());
        digest.update(field);
    }
    digest.finalize().into()
}

pub(crate) fn parse_identity(identity: &str) -> Result<[u8; 32], DomainRefusal> {
    if identity.len() != 64 {
        return Err(DomainRefusal::WrongBinding);
    }
    let mut bytes = [0; 32];
    for (index, pair) in identity.as_bytes().as_chunks::<2>().0.iter().enumerate() {
        let digit = |byte| match byte {
            b'0'..=b'9' => Ok(byte - b'0'),
            b'a'..=b'f' => Ok(byte - b'a' + 10),
            _ => Err(DomainRefusal::WrongBinding),
        };
        bytes[index] = digit(pair[0])? * 16 + digit(pair[1])?;
    }
    Ok(bytes)
}
