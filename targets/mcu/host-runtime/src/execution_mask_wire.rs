//! One exclusive codec buffer for successive Face-input and Show-result phases.
//! Renderer state and Rust borrows prevent simultaneous use; each phase retains
//! its own original wire ceiling even when the backing allocation is larger.
extern crate alloc;
use alloc::{boxed::Box, vec};
pub(super) struct Storage {
    bytes: Box<[u8]>,
    face: usize,
    show: usize,
}
impl Storage {
    pub(super) fn prepare(face: u32, show: u32) -> Result<Self, ()> {
        if face == 0 || show == 0 || face > 8192 || show > 8192 {
            return Err(());
        }
        Ok(Self {
            bytes: vec![0; face.max(show) as usize].into_boxed_slice(),
            face: face as usize,
            show: show as usize,
        })
    }
    pub(super) fn face_mut(&mut self) -> &mut [u8] {
        &mut self.bytes[..self.face]
    }
    pub(super) fn show_mut(&mut self) -> &mut [u8] {
        &mut self.bytes[..self.show]
    }
    pub(super) fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub(super) fn retained_bytes(&self) -> usize {
        self.bytes.len()
    }
    pub(super) fn clear(&mut self) {
        self.bytes.fill(0);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn successive_codecs_share_backing_but_keep_independent_extents() {
        let mut storage = Storage::prepare(6144, 4096).unwrap();
        assert_eq!(storage.retained_bytes(), 6144);
        let first = storage.face_mut().as_ptr();
        storage.face_mut()[6143] = 7;
        assert_eq!(storage.show_mut().len(), 4096);
        assert_eq!(storage.show_mut().as_ptr(), first);
        assert!(storage.show_mut().get_mut(4096).is_none());
        assert_eq!(storage.bytes()[6143], 7);
        storage.clear();
        assert!(storage.bytes().iter().all(|b| *b == 0));
        assert!(Storage::prepare(0, 4096).is_err());
        assert!(Storage::prepare(6144, 8193).is_err());
    }
}
