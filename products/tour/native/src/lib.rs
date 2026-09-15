//! Native Linux and Windows presentation adapter for the shared Tour application Form.

mod presenter;

pub use presenter::{DesktopPresentation, DesktopPresenter, DesktopPresenterRefusal};

/// Exact platform implementation advertised by this compiled desktop artifact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopPlatform {
    Linux,
    Windows,
}

impl DesktopPlatform {
    pub const fn current() -> Option<Self> {
        #[cfg(target_os = "linux")]
        {
            return Some(Self::Linux);
        }
        #[cfg(target_os = "windows")]
        {
            return Some(Self::Windows);
        }
        #[allow(unreachable_code)]
        None
    }

    pub const fn implementation_id(self) -> &'static str {
        match self {
            Self::Linux => "conduit-tour/linux-desktop-presentation@1",
            Self::Windows => "conduit-tour/windows-desktop-presentation@1",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supported_platforms_have_distinct_native_implementation_identity() {
        assert_ne!(
            DesktopPlatform::Linux.implementation_id(),
            DesktopPlatform::Windows.implementation_id()
        );
        assert!(DesktopPlatform::Linux.implementation_id().contains("linux"));
        assert!(
            DesktopPlatform::Windows
                .implementation_id()
                .contains("windows")
        );
    }
}
