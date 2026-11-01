//! Finite, allocation-free UF2 header admission for native deployment.
use crate::{classify_image_page, Refusal, BOOTSTRAP, ERASE_BYTES, FLASH_BYTES, PAGE_BYTES};

pub const UF2_BLOCK_BYTES: usize = 512;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Uf2Refusal {
    Extent,
    Header,
    Sequence,
    Sparse,
    Geometry(Refusal),
    Incomplete,
}
pub struct Uf2Validator {
    blocks: u32,
    next: u32,
    previous_end: Option<u32>,
}
fn word(block: &[u8; UF2_BLOCK_BYTES], at: usize) -> u32 {
    u32::from_le_bytes(block[at..at + 4].try_into().unwrap())
}
impl Uf2Validator {
    pub fn new(bytes: u64) -> Result<Self, Uf2Refusal> {
        if bytes == 0
            || bytes > u64::from(FLASH_BYTES) * 2
            || !bytes.is_multiple_of(UF2_BLOCK_BYTES as u64)
        {
            return Err(Uf2Refusal::Extent);
        }
        Ok(Self {
            blocks: (bytes / UF2_BLOCK_BYTES as u64) as u32,
            next: 0,
            previous_end: None,
        })
    }
    pub fn push(&mut self, block: &[u8; UF2_BLOCK_BYTES]) -> Result<(), Uf2Refusal> {
        if word(block, 0) != 0x0a32_4655
            || word(block, 4) != 0x9e5d_5157
            || word(block, 508) != 0x0ab1_6f30
            || word(block, 8) != 0x2000
            || word(block, 28) != 0xe48b_ff56
        {
            return Err(Uf2Refusal::Header);
        }
        if self.next >= self.blocks
            || word(block, 20) != self.next
            || word(block, 24) != self.blocks
        {
            return Err(Uf2Refusal::Sequence);
        }
        let address = word(block, 12);
        classify_image_page(address, word(block, 16)).map_err(Uf2Refusal::Geometry)?;
        if let Some(previous) = self.previous_end {
            if address < previous {
                return Err(Uf2Refusal::Sequence);
            }
            if address != previous && address != BOOTSTRAP.address() {
                return Err(Uf2Refusal::Sparse);
            }
        } else if !address.is_multiple_of(ERASE_BYTES) {
            return Err(Uf2Refusal::Geometry(Refusal::Alignment));
        }
        self.previous_end = Some(address + PAGE_BYTES);
        self.next += 1;
        Ok(())
    }
    pub fn finish(self) -> Result<(), Uf2Refusal> {
        if self.next == self.blocks {
            Ok(())
        } else {
            Err(Uf2Refusal::Incomplete)
        }
    }
}
