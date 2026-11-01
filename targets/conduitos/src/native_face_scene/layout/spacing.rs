//! Finite native row expression, separate from structural layout and input custody.
use super::*;

/// Renderer-owned parts of an existing native component. This is not a selector
/// grammar, Face role, action grant, or public style-hook registry.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::native_face_scene) enum NativePart {
    Content,
    Control,
}
#[derive(Clone, Copy)]
pub(in crate::native_face_scene) struct RowInsets {
    pub inline: u16,
    pub block: u16,
    /// Reserved native focus marker gutter; it is not an authored padding token.
    pub leading: u16,
}
#[derive(Clone, Copy)]
pub(in crate::native_face_scene) struct RowSpacing {
    pub padding: u16,
    pub gap: u16,
    pub control: Option<RowInsets>,
}
impl RowSpacing {
    pub fn legacy(padding: u16) -> Self {
        Self {
            padding,
            gap: SPACE_SM,
            control: None,
        }
    }
    pub fn validate(self) -> Result<Self, FaceSceneError> {
        if !(SPACE_SM..=SPACE_SM * 2).contains(&self.padding)
            || self.gap > 32
            || self
                .control
                .is_some_and(|i| i.inline > 64 || i.block > 64 || i.leading != 8)
        {
            return Err(FaceSceneError::InvalidExtent);
        }
        Ok(self)
    }
    pub fn insets(self, item: &document::Item) -> RowInsets {
        if item.part == NativePart::Control
            && let Some(insets) = self.control
        {
            return insets;
        }
        RowInsets {
            inline: self.padding,
            block: self.padding,
            leading: 0,
        }
    }
    pub fn box_height(
        self,
        item: &document::Item,
        measured: u16,
        scale: u16,
    ) -> Result<u16, FaceSceneError> {
        let icon = if item.icon.is_some() {
            super::super::semantic_icons::size(scale)
        } else {
            0
        };
        let choice = if item.checkbox.is_some() { 18 } else { 0 };
        measured
            .max(icon)
            .max(choice)
            .checked_add(self.insets(item).block * 2)
            .ok_or(FaceSceneError::InvalidExtent)
    }
}

/// Internal preparation storage only: two seven-bit admitted values and one
/// fixed-gutter flag share the original two-byte row-padding slot. Not Face wire.
#[derive(Clone, Copy)]
pub(in crate::native_face_scene) struct PackedRowInsets(u16);
impl PackedRowInsets {
    pub fn new(insets: RowInsets) -> Result<Self, FaceSceneError> {
        if insets.inline > 64 || insets.block > 64 || !matches!(insets.leading, 0 | 8) {
            return Err(FaceSceneError::InvalidExtent);
        }
        Ok(Self(
            insets.inline | (insets.block << 7) | (u16::from(insets.leading != 0) << 14),
        ))
    }
    pub fn inline(self) -> u16 {
        self.0 & 0x7f
    }
    pub fn block(self) -> u16 {
        (self.0 >> 7) & 0x7f
    }
    pub fn leading(self) -> u16 {
        ((self.0 >> 14) & 1) * 8
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn packed_insets_preserve_every_admitted_value_in_the_original_storage_slot() {
        assert_eq!(
            core::mem::size_of::<PackedRowInsets>(),
            core::mem::size_of::<u16>()
        );
        for inline in 0..=64 {
            for block in 0..=64 {
                for leading in [0, 8] {
                    let packed = PackedRowInsets::new(RowInsets {
                        inline,
                        block,
                        leading,
                    })
                    .unwrap();
                    assert_eq!(
                        (packed.inline(), packed.block(), packed.leading()),
                        (inline, block, leading)
                    );
                }
            }
        }
        for (inline, block, leading) in [(65, 0, 0), (0, 65, 0), (0, 0, 1), (0, 0, 16)] {
            assert!(
                PackedRowInsets::new(RowInsets {
                    inline,
                    block,
                    leading
                })
                .is_err()
            );
        }
    }
}
