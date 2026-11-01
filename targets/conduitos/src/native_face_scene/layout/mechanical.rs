//! Preparation-only row/column/grid realization. Geometry never becomes reading order.
//! Two candidates, 32 nodes and depth four are checked by conduit-face. Each
//! arrangement retains at most MAX_ROWS rows and MAX_DOCUMENT_BYTES text bytes;
//! composition moves rows rather than copying them. Score/discard at most two
//! candidates, then prepare the winner once. Grid continuation keeps finite
//! span-connected bands intact; oversized bands explicitly refuse.
use super::*;
use crate::display::typography::{TextRole, intrinsic_item_text_metrics, resolve_em_sixteenths};
use conduit_face::{
    ComponentLayoutAlignment as Alignment, ComponentLayoutBreak as Break, ComponentLayoutCandidate,
    ComponentLayoutChild, ComponentLayoutNode, PreparedComponentLayout,
};
#[path = "mechanical/compose.rs"]
mod compose;
#[path = "mechanical/fragment.rs"]
mod fragment;
#[path = "mechanical/grid.rs"]
mod grid;
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
    let mut best: Option<(usize, u64, usize)> = None;
    let mut refusal = FaceSceneError::InvalidExtent;
    for (index, candidate) in prepared.structure().candidates().iter().enumerate() {
        match arrange_candidate(&context, candidate, height) {
            Ok(fragments) => {
                let score = (
                    fragments.len(),
                    fragments.iter().map(|f| u64::from(f.height)).sum(),
                );
                if best.as_ref().is_none_or(|prior| score < (prior.0, prior.1)) {
                    best = Some((score.0, score.1, index));
                }
                // Keep only the score. Retaining a prior candidate's rows would
                // make its document storage coexist with the next candidate.
            }
            Err(error) => refusal = error,
        }
    }
    let (_, _, index) = best.ok_or(refusal)?;
    let fragments = arrange_candidate(
        &context,
        &prepared.structure().candidates().as_slice()[index],
        height,
    )?;
    let mut entries = Vec::new();
    for (page, fragment) in fragments.into_iter().enumerate() {
        // Metadata follows the final page through the existing reading-order
        // sort; frame reporting already visits every row on that page.
        let mut continuation = if page == 0 {
            None
        } else {
            fragment.continuation
        };
        for mut entry in fragment.entries {
            entry.row.continuation = continuation.take();
            entry.row.page = page;
            entry.row.bounds.y += chrome.top as i16;
            entries.push(entry);
        }
    }
    entries.sort_unstable_by_key(|entry| (entry.original, entry.chunk));
    Ok(entries.into_iter().map(|entry| entry.row).collect())
}

fn arrange_candidate(
    context: &Context<'_>,
    candidate: &ComponentLayoutCandidate,
    height: u16,
) -> Result<Vec<Fragment>, FaceSceneError> {
    context.rows.set(0);
    let mut demands = [Demand::default(); 32];
    structure::measure(
        context,
        candidate,
        usize::from(*candidate.root()),
        &mut demands,
    )?;
    compose::arrange(
        context,
        candidate,
        usize::from(*candidate.root()),
        context.width,
        height,
        &demands,
    )
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

#[cfg(test)]
#[path = "mechanical/grid_fixture.rs"]
mod grid_fixture;
#[cfg(test)]
#[path = "mechanical/grid_tests.rs"]
mod grid_tests;

#[cfg(test)]
#[path = "mechanical/minimum_tests.rs"]
mod minimum_tests;

#[cfg(test)]
#[path = "mechanical/control_cluster_tests.rs"]
mod control_cluster_tests;
