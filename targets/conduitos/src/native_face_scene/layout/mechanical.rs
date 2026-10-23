//! Preparation-only row/column realization. Geometry never becomes reading order.
//! Two candidates, 32 nodes and depth four are checked by conduit-face. Each
//! candidate retains at most MAX_ROWS rows and MAX_DOCUMENT_BYTES text bytes;
//! composition moves rows rather than copying them. Both candidates are evaluated
//! sequentially; at most the current and best result coexist. Grid is refused.
use super::*;
use crate::display::typography::{TextRole, intrinsic_text_metrics, resolve_em_sixteenths};
use conduit_face::{
    ComponentLayoutAlignment as Alignment, ComponentLayoutBreak as Break, ComponentLayoutCandidate,
    ComponentLayoutChild, ComponentLayoutNode, PreparedComponentLayout,
};
#[path = "mechanical/compose.rs"]
mod compose;
#[path = "mechanical/fragment.rs"]
mod fragment;
#[path = "mechanical/leaves.rs"]
mod leaves;
#[path = "mechanical/structure.rs"]
mod structure;
use fragment::{Entry, Fragment};
use structure::{Demand, Group, Node};

pub(in crate::native_face_scene) fn admit(
    items: Vec<document::Item>,
    screen: LayoutRect,
    prepared: &PreparedComponentLayout<'_>,
    scale: u16,
    padding: u16,
    typography: Typography,
) -> Result<Vec<Row>, FaceSceneError> {
    field_clusters::validate(&items)?;
    let chrome = chrome::Chrome::prepare_selected(screen, scale, typography)?;
    let height = chrome
        .bottom
        .checked_sub(chrome.top)
        .filter(|h| *h > 0)
        .ok_or(FaceSceneError::InvalidExtent)?;
    let context = Context {
        items: &items,
        width: screen.width,
        scale,
        padding,
        typography,
        rows: core::cell::Cell::new(0),
    };
    let mut best: Option<(usize, u64, Vec<Fragment>)> = None;
    let mut refusal = FaceSceneError::InvalidExtent;
    for candidate in prepared.structure().candidates().iter() {
        context.rows.set(0);
        let attempt = (|| {
            let mut demands = [Demand::default(); 32];
            structure::measure(
                &context,
                candidate,
                usize::from(*candidate.root()),
                &mut demands,
            )?;
            let fragments = compose::arrange(
                &context,
                candidate,
                usize::from(*candidate.root()),
                screen.width,
                height,
                &demands,
            )?;
            let block: u64 = fragments.iter().map(|f| u64::from(f.height)).sum();
            Ok::<_, FaceSceneError>((fragments.len(), block, fragments))
        })();
        match attempt {
            Ok(result)
                if best
                    .as_ref()
                    .is_none_or(|prior| (result.0, result.1) < (prior.0, prior.1)) =>
            {
                best = Some(result)
            }
            Ok(_) => (),
            Err(error) => refusal = error,
        }
    }
    let (_, _, fragments) = best.ok_or(refusal)?;
    let mut entries = Vec::new();
    for (page, fragment) in fragments.into_iter().enumerate() {
        for mut entry in fragment.entries {
            entry.row.page = page;
            entry.row.bounds.y += chrome.top as i16;
            entries.push(entry);
        }
    }
    entries.sort_unstable_by_key(|entry| (entry.original, entry.chunk));
    Ok(entries.into_iter().map(|entry| entry.row).collect())
}

struct Context<'a> {
    items: &'a [document::Item],
    width: u16,
    scale: u16,
    padding: u16,
    typography: Typography,
    rows: core::cell::Cell<usize>,
}
impl Context<'_> {
    fn em(&self, units: u16) -> Result<u16, FaceSceneError> {
        resolve_em_sixteenths(self.typography.style(TextRole::Body), self.scale, units)
            .map_err(|_| FaceSceneError::InvalidExtent)
    }
    fn items(
        &self,
        region: Option<usize>,
    ) -> impl Iterator<Item = (usize, &document::Item)> + Clone {
        self.items
            .iter()
            .enumerate()
            .filter(move |(_, item)| item.region_order == region)
    }
}

fn add(a: u16, b: u16) -> Result<u16, FaceSceneError> {
    a.checked_add(b).ok_or(FaceSceneError::InvalidExtent)
}
fn inset(width: u16, padding: u16) -> Result<u16, FaceSceneError> {
    width
        .checked_sub(
            padding
                .checked_mul(2)
                .ok_or(FaceSceneError::InvalidExtent)?,
        )
        .filter(|width| *width > 0)
        .ok_or(FaceSceneError::InvalidExtent)
}
fn offset(alignment: &Alignment, available: u16, used: u16) -> u16 {
    let spare = available.saturating_sub(used);
    match alignment {
        Alignment::Center => spare / 2,
        Alignment::End => spare,
        _ => 0,
    }
}
#[cfg(test)]
#[path = "mechanical/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "mechanical/fragment_tests.rs"]
mod fragment_tests;
