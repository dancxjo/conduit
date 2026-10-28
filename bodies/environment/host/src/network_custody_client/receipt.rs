//! Verification of adopted custody, never permission to delete local evidence.
use super::*;
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Response {
    profile: String,
    reclaimed: bool,
    generation: u64,
    receipt: String,
}
/// Actual Root-signed cursor for the exact retained offer and member ACK.
/// HTTP's unsigned generation is deliberately not an authoritative watermark.
pub(crate) struct CommittedNetworkCustody {
    cursor: [u8; 480],
}
impl CommittedNetworkCustody {
    pub(crate) fn bytes(&self) -> &[u8; 480] {
        &self.cursor
    }
}
fn hex(text: &str) -> Result<[u8; 480]> {
    if text.len() != 960 {
        return Err("network receipt extent".into());
    }
    let mut bytes = [0; 480];
    fn digit(c: u8) -> Result<u8> {
        match c {
            b'0'..=b'9' => Ok(c - b'0'),
            b'a'..=b'f' => Ok(c - b'a' + 10),
            _ => Err("network receipt hex".into()),
        }
    }
    for (out, pair) in bytes.iter_mut().zip(text.as_bytes().chunks_exact(2)) {
        *out = digit(pair[0])? * 16 + digit(pair[1])?;
    }
    Ok(bytes)
}
impl VerifiedDurableSnapshot {
    pub(crate) fn verify_committed_receipt(
        &self,
        credential: &MembershipCredential,
        acknowledgement: &[u8],
        response: &[u8],
    ) -> Result<CommittedNetworkCustody> {
        if response.len() > 1408 {
            return Err("network receipt response capacity".into());
        }
        let response: Response = serde_json::from_slice(response)?;
        if response.profile != "network-retirement-committed@1"
            || response.reclaimed
            || response.generation == 0
        {
            return Err("network receipt response profile".into());
        }
        self.revalidate()?;
        if acknowledgement.len() != REQUEST {
            return Err("network ACK extent".into());
        }
        let signature: [u8; 64] = acknowledgement[564..].try_into().unwrap();
        if self.encode_fields(credential, signature)?.as_slice() != acknowledgement {
            return Err("network receipt ACK identity".into());
        }
        let cursor = hex(&response.receipt)?;
        let epoch = match cursor[8] {
            1 => self.scope.epoch,
            2 => self
                .scope
                .epoch
                .checked_add(1)
                .ok_or("network receipt epoch overflow")?,
            _ => return Err("network receipt phase".into()),
        };
        let message = self.signing_transcript(credential)?;
        if &cursor[..8] != b"NCURSOR1"
            || cursor[9..16]
                .iter()
                .chain(cursor[392..408].iter())
                .chain(cursor[472..].iter())
                .any(|b| *b != 0)
            || cursor[16..24] != epoch.to_le_bytes()
            || cursor[168..200] != self.offer
            || cursor[200..232] != self.member_scope(credential)?
            || cursor[264..328] != signature
            || cursor[328..360] != message
            || cursor[360..392] != self.scope.predecessor
        {
            return Err("network receipt scope".into());
        }
        // Reconstruct the complete original offer from the receipt. Equality to
        // the durably retained offer binds Body/region/Boot/count/full digest.
        let mut offered = Sha256::new();
        offered.update(b"conduit.environment/network-retirement-offer@1\0");
        offered.update(self.scope.root);
        offered.update(&cursor[24..168]);
        offered.update(&cursor[360..392]);
        if <[u8; 32]>::from(offered.finalize()) != self.offer {
            return Err("network receipt retained offer".into());
        }
        checked(VerifyingKey::from_bytes(
            cursor[232..264].try_into().unwrap(),
        ))?
        .verify_strict(&message, &Signature::from_bytes(&signature))
        .map_err(|_| "network receipt member signature")?;
        let mut signed = Sha256::new();
        signed.update(b"conduit.environment/network-retirement-receipt@1\0");
        signed.update(&cursor[..408]);
        checked(VerifyingKey::from_bytes(&self.scope.root))?
            .verify_strict(
                &signed.finalize(),
                &Signature::from_bytes(cursor[408..472].try_into().unwrap()),
            )
            .map_err(|_| "network receipt Root signature")?;
        Ok(CommittedNetworkCustody { cursor })
    }
}
