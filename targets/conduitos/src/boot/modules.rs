//! Bounded selection of bootloader-retained artifact bytes, without admission.
use super::MAX_ARTIFACTS;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BootModuleRefusal {
    InvalidSelection,
    ArtifactBound,
    Duplicate,
    PayloadBound,
}

pub(crate) struct ObservedModule<'a> {
    pub command: &'a [u8],
    pub bytes: &'a [u8],
}

pub(crate) fn select_named<'a>(
    modules: impl IntoIterator<Item = ObservedModule<'a>>,
    command: &[u8],
    maximum_bytes: usize,
) -> Result<Option<&'a [u8]>, BootModuleRefusal> {
    if command.is_empty() || maximum_bytes == 0 {
        return Err(BootModuleRefusal::InvalidSelection);
    }
    let mut selected = None;
    for (index, module) in modules.into_iter().enumerate() {
        if index >= MAX_ARTIFACTS {
            return Err(BootModuleRefusal::ArtifactBound);
        }
        if module.command != command {
            continue;
        }
        if selected.is_some() {
            return Err(BootModuleRefusal::Duplicate);
        }
        if module.bytes.is_empty() || module.bytes.len() > maximum_bytes {
            return Err(BootModuleRefusal::PayloadBound);
        }
        selected = Some(module.bytes);
    }
    Ok(selected)
}

#[cfg(test)]
mod tests {
    use super::*;
    const NAME: &[u8] = b"conduit.protocol/source@1";
    fn observed<'a>(command: &'a [u8], bytes: &'a [u8]) -> ObservedModule<'a> {
        ObservedModule { command, bytes }
    }

    #[test]
    fn selection_borrows_only_the_exact_named_module() {
        let bytes = b"retained";
        let selected = select_named(
            [observed(b"other", b"unrelated"), observed(NAME, bytes)],
            NAME,
            bytes.len(),
        )
        .unwrap()
        .unwrap();
        assert!(core::ptr::eq(selected.as_ptr(), bytes.as_ptr()));
        assert_eq!(
            select_named([observed(b"other", bytes)], NAME, 16),
            Ok(None)
        );
    }

    #[test]
    fn duplicate_payload_and_inventory_bounds_have_distinct_refusals() {
        assert_eq!(
            select_named([observed(NAME, b"a"), observed(NAME, b"b")], NAME, 1),
            Err(BootModuleRefusal::Duplicate)
        );
        for bytes in [&b""[..], &b"too long"[..]] {
            assert_eq!(
                select_named([observed(NAME, bytes)], NAME, 1),
                Err(BootModuleRefusal::PayloadBound)
            );
        }
        assert_eq!(
            select_named(
                (0..=MAX_ARTIFACTS).map(|_| observed(b"other", b"")),
                NAME,
                1
            ),
            Err(BootModuleRefusal::ArtifactBound)
        );
        assert_eq!(
            select_named([], NAME, 0),
            Err(BootModuleRefusal::InvalidSelection)
        );
    }
}
