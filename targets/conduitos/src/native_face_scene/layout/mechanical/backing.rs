//! Requested backing of the existing finite mechanical containers. Rows move;
//! scoring drops each candidate before the next candidate or winning replay.
use super::*;
use crate::native_face_scene::FaceSceneError;
use core::mem::size_of;

pub(in crate::native_face_scene) struct MechanicalBacking {
    pub retained_rows: usize,
    pub preparation_peak: usize,
}
fn sum(a: usize, b: usize) -> Result<usize, FaceSceneError> {
    a.checked_add(b).ok_or(FaceSceneError::DocumentBound)
}
fn product(a: usize, b: usize) -> Result<usize, FaceSceneError> {
    a.checked_mul(b).ok_or(FaceSceneError::DocumentBound)
}
// These element types are not bytes. Rust1.98 RawVec's minimum nonzero capacity
// is at most4; geometric grow/extend reserves at most max(4,2*final length).
fn slots(length: usize) -> Result<usize, FaceSceneError> {
    if length == 0 {
        Ok(0)
    } else {
        Ok(product(length, 2)?.max(4))
    }
}
fn aggregate_entry_slots(rows: usize) -> Result<usize, FaceSceneError> {
    // Every nonempty Fragment consumes at least one row. Summing max(4,2*n)
    // over its entry vectors is <=4*total rows, even for one-row pages.
    product(rows, 4)
}
pub(in crate::native_face_scene) fn memory_bounds(
    prepared: &PreparedComponentLayout<'_>,
    primary_text: usize,
) -> Result<MechanicalBacking, FaceSceneError> {
    let text = primary_text.min(MAX_DOCUMENT_BYTES);
    // A nonempty text chunk consumes >=1 source byte; the renderer refuses
    // before adding row MAX_ROWS+1. Empty groups allocate no row text.
    let rows = text.min(MAX_ROWS);
    let entries = product(aggregate_entry_slots(rows)?, size_of::<Entry>())?;
    let fragments = product(slots(rows)?, size_of::<Fragment>())?;
    // collect(map Entry->Row) may reuse the larger Entry allocation. Its
    // retained bytes must not be inferred from Vec<Row>'s element count alone.
    let retained_rows = product(slots(rows)?, size_of::<Entry>().max(size_of::<Row>()))?;
    let mut peak = 0;
    for candidate in prepared.structure().candidates() {
        let mut arrangement = 0;
        let mut maximum_children = 0;
        let mut grid_band = 0;
        for node in candidate.nodes() {
            // Each node may retain its child iterator arrays while assembling
            // its own output. Charge their actual checked node count, not depth
            // times an invented row copy. Text payload itself has one owner.
            arrangement = sum(arrangement, sum(entries, fragments)?)?;
            let (children, row, grid) = match node {
                ComponentLayoutNode::Region(_) | ComponentLayoutNode::Fallback => (0, false, false),
                ComponentLayoutNode::Row(group) => (group.children().len(), true, false),
                ComponentLayoutNode::Column(group) => (group.children().len(), false, false),
                ComponentLayoutNode::Grid(value) => (value.group().children().len(), false, true),
            };
            maximum_children = maximum_children.max(children);
            if row {
                type Lane = core::iter::Peekable<alloc::vec::IntoIter<Fragment>>;
                arrangement = sum(arrangement, product(slots(children)?, size_of::<Lane>())?)?;
            }
            if grid {
                arrangement = sum(
                    arrangement,
                    product(slots(children)?, grid::cell_inline_bytes())?,
                )?;
                // The selected band indexes are collected before cells move.
                arrangement = sum(arrangement, product(slots(children)?, size_of::<usize>())?)?;
                // A rejected band can temporarily join every one-page cell
                // before its 20-row/command gate. That pre-refusal Vec matters.
                let band_rows = rows.min(product(children, MAX_PAGE_ROWS)?);
                grid_band = grid_band.max(product(slots(band_rows)?, size_of::<Entry>())?);
            }
        }
        arrangement = sum(arrangement, grid_band)?;
        // take_prefix creates one <=20-entry tail before moving its old buffer.
        // No child recursion occurs during this split loop.
        if rows != 0 {
            arrangement = sum(
                arrangement,
                product(MAX_PAGE_ROWS.min(rows), size_of::<Entry>())?,
            )?;
        }
        // Old+new realloc overlap: one grow at a time, separately named for
        // entry, fragment, lane/cell and selected-index containers.
        arrangement = sum(
            arrangement,
            product(rows, size_of::<Entry>() + size_of::<Fragment>())?,
        )?;
        type Lane = core::iter::Peekable<alloc::vec::IntoIter<Fragment>>;
        arrangement = sum(
            arrangement,
            product(
                maximum_children,
                size_of::<Lane>() + grid::cell_inline_bytes() + size_of::<usize>(),
            )?,
        )?;
        // Winning flatten holds the original fragments, a growing Entry Vec,
        // and (unless in-place collection succeeds) the destination Row Vec.
        let flatten = sum(
            sum(entries, fragments)?,
            sum(
                product(slots(rows)?, size_of::<Entry>())?,
                sum(retained_rows, product(rows, size_of::<Entry>())?)?,
            )?,
        )?;
        peak = peak.max(sum(text, arrangement.max(flatten))?);
    }
    Ok(MechanicalBacking {
        retained_rows,
        preparation_peak: peak,
    })
}
