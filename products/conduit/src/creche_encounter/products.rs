//! Retained Seed output preserves its selected preparation profile.
use super::*;
use std::io::{Read, Seek, SeekFrom};

pub enum CrecheProducts {
    Reference(ProducedJoiningHost<File>),
    Image(ImageCrecheProducts),
}
impl CrecheProducts {
    pub fn products(&self) -> &ProducedJoiningHost<File> {
        match self {
            Self::Reference(products) => products,
            Self::Image(products) => products.products(),
        }
    }
    /// Borrow the artifact cursor without permitting replacement or extracting
    /// its owning Seed metadata from the retained IMAGE charge.
    pub fn artifact_reader(&mut self) -> impl Read + Seek + '_ {
        ArtifactReader(match self {
            Self::Reference(products) => &mut products.artifact,
            Self::Image(products) => &mut products.products.artifact,
        })
    }
}
struct ArtifactReader<'a>(&'a mut File);
impl Read for ArtifactReader<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(bytes)
    }
}
impl Seek for ArtifactReader<'_> {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.0.seek(position)
    }
}
impl CrecheEncounterController {
    /// Transfer the published result with its existing profile and custody.
    pub fn take_retained_products(&self) -> Result<Option<CrecheProducts>, String> {
        if self.channel.is_image() {
            self.take_image_products()
                .map(|value| value.map(CrecheProducts::Image))
        } else {
            self.take_products()
                .map(|value| value.map(CrecheProducts::Reference))
        }
    }
}
