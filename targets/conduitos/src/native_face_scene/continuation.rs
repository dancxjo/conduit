//! Mask realization metadata, never a source fact or authority grant.
use core::num::NonZeroU8;

/// All pressures that forced one page boundary, plus its structural decision.
/// One niche-packed byte per prepared row; no extra collection or search pass.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(transparent)]
pub struct LayoutContinuation(NonZeroU8);
impl LayoutContinuation {
    const BLOCK: u8 = 1;
    const ROWS: u8 = 2;
    const COMMANDS: u8 = 4;
    const KEEP: u8 = 8;
    const MEASUREMENT: u8 = 16;

    pub(super) fn pressure(block: bool, rows: bool, commands: bool) -> Option<Self> {
        NonZeroU8::new(u8::from(block) | (u8::from(rows) << 1) | (u8::from(commands) << 2))
            .map(Self)
    }
    pub(super) fn measurement_refusal() -> Self {
        Self(NonZeroU8::new(Self::MEASUREMENT).unwrap())
    }
    pub(super) fn keeping_group(self) -> Self {
        Self(NonZeroU8::new(self.0.get() | Self::KEEP).unwrap())
    }
    pub const fn block_limit(self) -> bool {
        self.0.get() & Self::BLOCK != 0
    }
    pub const fn row_limit(self) -> bool {
        self.0.get() & Self::ROWS != 0
    }
    /// Reserved command demand, including focus/state reserves, exceeded the
    /// page budget. Actual emission may be smaller than its admitted reserve.
    pub const fn command_limit(self) -> bool {
        self.0.get() & Self::COMMANDS != 0
    }
    pub const fn kept_group(self) -> bool {
        self.0.get() & Self::KEEP != 0
    }
    pub const fn measurement_refused(self) -> bool {
        self.0.get() & Self::MEASUREMENT != 0
    }
}
const _: () = assert!(core::mem::size_of::<Option<LayoutContinuation>>() == 1);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_face_scene::{FaceControl, document, layout};
    use alloc::{format, vec::Vec};
    use conduit_face::{GraphicsPaintRole, GraphicsTextRole, LayoutRect};

    fn rows(count: usize, height: u16, controls: bool) -> Vec<layout::Row> {
        let items = (0..count)
            .map(|n| document::Item {
                text: format!("value {n}"),
                role: if controls {
                    GraphicsTextRole::Heading
                } else {
                    GraphicsTextRole::Body
                },
                paint: GraphicsPaintRole::Foreground,
                control: controls.then_some(FaceControl {
                    action: n,
                    argument: None,
                }),
                indent: 0,
                subject_index: None,
                region: None,
                region_order: None,
                checkbox: None,
                icon: None,
            })
            .collect();
        layout::admit_with_expression(
            items,
            LayoutRect {
                x: 0,
                y: 0,
                width: 1280,
                height,
            },
            12,
            1,
            100,
            crate::display::SPACE_SM,
        )
        .unwrap()
    }
    #[test]
    fn row_pressure_is_not_reported_as_geometry_or_command_shortage() {
        let rows = rows(21, 2000, false);
        assert!(
            rows.iter()
                .filter(|r| r.page == 0)
                .all(|r| r.continuation.is_none())
        );
        let cause = rows[20].continuation.unwrap();
        assert!(cause.row_limit());
        assert!(!cause.block_limit());
        assert!(!cause.command_limit());
        assert!(!cause.kept_group());
        assert!(!cause.measurement_refused());
    }
    #[test]
    fn geometric_pressure_with_spare_row_and_command_capacity_is_explicit() {
        let rows = rows(12, 240, false);
        let cause = rows.iter().find_map(|r| r.continuation).unwrap();
        assert!(cause.block_limit());
        assert!(!cause.row_limit());
        assert!(!cause.command_limit());
    }
    #[test]
    fn command_pressure_can_break_with_spare_geometry_and_row_capacity() {
        let rows = rows(18, 2000, true);
        let cause = rows.iter().find_map(|row| row.continuation).unwrap();
        assert!(cause.command_limit());
        assert!(!cause.row_limit() && !cause.block_limit() && !cause.kept_group());
    }

    #[test]
    fn independent_pressures_are_retained_instead_of_using_a_priority_guess() {
        let cause = LayoutContinuation::pressure(true, true, true)
            .unwrap()
            .keeping_group();
        assert!(
            cause.block_limit() && cause.row_limit() && cause.command_limit() && cause.kept_group()
        );
        assert!(LayoutContinuation::pressure(false, false, false).is_none());
        let width = LayoutContinuation::measurement_refusal();
        assert!(width.measurement_refused());
        assert!(!width.block_limit() && !width.command_limit() && !width.row_limit());
    }
    #[test]
    fn frame_exposes_continuation_without_replacing_face_or_control_bindings() {
        let face = crate::native_face_scene::tests::face(7);
        let mut scene =
            crate::native_face_scene::NativeFaceScene::prepare(face.clone(), 320, 240).unwrap();
        assert!(scene.pages() > 1);
        for page in 0..scene.pages() {
            let frame = scene.frame().unwrap();
            assert_eq!(frame.page, page);
            assert_eq!(frame.continuation.is_some(), page != 0);
            assert_eq!(frame.face_id, &face.identity);
            assert!(frame.scene.commands().len() <= conduit_face::MAX_GRAPHICS_COMMANDS);
            for hit in &frame.hits {
                assert_eq!(hit.action, &face.actions[hit.control.action]);
            }
            scene.turn_page(true);
        }
        assert_eq!(scene.face(), &face);
    }
}
