//! Native artifact contracts for fabrication outputs.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SporeOutputKind {
    NativeBundle,
    BrowserBundle,
    IntelHex,
    Uf2,
    DiskImage,
    EfiArtifact,
    Esp32Image,
    SdImage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeSporePackaging {
    SingleFile,
    Zip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeSporeArtifactContract {
    pub extension: &'static str,
    pub media_type: &'static str,
    pub packaging: NativeSporePackaging,
}

impl SporeOutputKind {
    pub const fn native_artifact_contract(&self) -> NativeSporeArtifactContract {
        use NativeSporePackaging::{SingleFile, Zip};
        match self {
            Self::NativeBundle | Self::BrowserBundle => NativeSporeArtifactContract {
                extension: "zip",
                media_type: "application/zip",
                packaging: Zip,
            },
            Self::IntelHex => NativeSporeArtifactContract {
                extension: "hex",
                media_type: "text/x-ihex",
                packaging: SingleFile,
            },
            Self::Uf2 => NativeSporeArtifactContract {
                extension: "uf2",
                media_type: "application/x-uf2",
                packaging: SingleFile,
            },
            Self::DiskImage => NativeSporeArtifactContract {
                extension: "iso",
                media_type: "application/x-iso9660-image",
                packaging: SingleFile,
            },
            Self::EfiArtifact => NativeSporeArtifactContract {
                extension: "efi",
                media_type: "application/vnd.microsoft.portable-executable",
                packaging: SingleFile,
            },
            Self::Esp32Image => NativeSporeArtifactContract {
                extension: "bin",
                media_type: "application/octet-stream",
                packaging: SingleFile,
            },
            Self::SdImage => NativeSporeArtifactContract {
                extension: "img",
                media_type: "application/x-raw-disk-image",
                packaging: SingleFile,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_spore_output_has_one_exact_native_artifact_contract() {
        let cases = [
            (
                SporeOutputKind::NativeBundle,
                "zip",
                "application/zip",
                NativeSporePackaging::Zip,
            ),
            (
                SporeOutputKind::BrowserBundle,
                "zip",
                "application/zip",
                NativeSporePackaging::Zip,
            ),
            (
                SporeOutputKind::IntelHex,
                "hex",
                "text/x-ihex",
                NativeSporePackaging::SingleFile,
            ),
            (
                SporeOutputKind::Uf2,
                "uf2",
                "application/x-uf2",
                NativeSporePackaging::SingleFile,
            ),
            (
                SporeOutputKind::DiskImage,
                "iso",
                "application/x-iso9660-image",
                NativeSporePackaging::SingleFile,
            ),
            (
                SporeOutputKind::EfiArtifact,
                "efi",
                "application/vnd.microsoft.portable-executable",
                NativeSporePackaging::SingleFile,
            ),
            (
                SporeOutputKind::Esp32Image,
                "bin",
                "application/octet-stream",
                NativeSporePackaging::SingleFile,
            ),
            (
                SporeOutputKind::SdImage,
                "img",
                "application/x-raw-disk-image",
                NativeSporePackaging::SingleFile,
            ),
        ];
        for (kind, extension, media_type, packaging) in cases {
            assert_eq!(
                kind.native_artifact_contract(),
                NativeSporeArtifactContract {
                    extension,
                    media_type,
                    packaging
                }
            );
        }
    }
}
