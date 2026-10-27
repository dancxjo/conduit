//! One streaming flavor for Body postcard SHA-256 transcripts. Callers retain
//! custody of domain prefixes and payload schemas; this grants no authority.
use sha2::{Digest, Sha256};

pub(crate) struct PostcardSha256(pub(crate) Sha256);

impl postcard::ser_flavors::Flavor for PostcardSha256 {
    type Output = [u8; 32];

    fn try_push(&mut self, byte: u8) -> postcard::Result<()> {
        self.0.update([byte]);
        Ok(())
    }

    fn try_extend(&mut self, bytes: &[u8]) -> postcard::Result<()> {
        self.0.update(bytes);
        Ok(())
    }

    fn finalize(self) -> postcard::Result<Self::Output> {
        Ok(self.0.finalize().into())
    }
}
