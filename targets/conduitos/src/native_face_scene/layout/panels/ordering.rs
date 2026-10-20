//! Checked recipe order governs geometry; original indices retain source reading.
use super::*;

pub(super) fn prepare(
    items: Vec<document::Item>,
    groups: [u8; 2],
) -> Result<Vec<(usize, document::Item)>, FaceSceneError> {
    if items.len() > MAX_ROWS {
        return Err(FaceSceneError::DocumentBound);
    }
    items.iter().try_fold(0usize, |bytes, item| {
        bytes
            .checked_add(item.text.len())
            .filter(|bytes| *bytes <= MAX_DOCUMENT_BYTES)
            .ok_or(FaceSceneError::DocumentBound)
    })?;
    let first = items.iter().position(|item| item.region.is_some());
    // All mandatory preparation work is admitted above; move the owned strings.
    let mut indexed: Vec<_> = items.into_iter().enumerate().collect();
    indexed.sort_unstable_by_key(|(original, item)| {
        let section = match item.region {
            None if first.is_none_or(|first| *original < first) => 0,
            Some(group) if group == groups[0] => 1,
            Some(group) if group == groups[1] => 2,
            Some(_) => 3,
            None => 4,
        };
        (section, item.region_order.unwrap_or(usize::MAX), *original)
    });
    Ok(indexed)
}

/// Sort only three machine words per row, then apply the permutation in place.
/// Each cycle uses bounded linear swaps; text buffers are never copied.
pub(super) fn restore(rows: &mut [Row], order: &mut [(usize, usize, usize)]) {
    order.sort_unstable_by_key(|(original, chunk, _)| (*original, *chunk));
    for start in 0..order.len() {
        let mut current = start;
        while order[current].2 != start {
            let next = order[current].2;
            rows.swap(current, next);
            order[current].2 = current;
            current = next;
        }
        order[current].2 = current;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn item(text: &str) -> document::Item {
        document::Item {
            text: text.into(),
            role: GraphicsTextRole::Body,
            paint: GraphicsPaintRole::Foreground,
            control: None,
            indent: 0,
            subject_index: None,
            region: None,
            region_order: None,
            checkbox: None,
        }
    }
    #[test]
    fn restoration_handles_three_cycle_fixed_point_and_original_item_chunks() {
        let mut rows = admit_with_expression(
            vec![item("c"), item("a"), item("b"), item("fixed")],
            LayoutRect {
                x: 0,
                y: 0,
                width: 1280,
                height: 800,
            },
            12,
            1,
            100,
            SPACE_SM,
        )
        .unwrap();
        let mut order = [(1, 1, 0), (0, 0, 1), (1, 0, 2), (2, 0, 3)];
        restore(&mut rows, &mut order);
        assert_eq!(
            rows.iter().map(|row| row.text.as_str()).collect::<Vec<_>>(),
            ["a", "b", "c", "fixed"]
        );
        assert!(
            order
                .iter()
                .enumerate()
                .all(|(index, entry)| entry.2 == index)
        );
    }
    #[test]
    fn ordering_refuses_count_and_text_before_permutation_allocation() {
        let too_many = (0..=MAX_ROWS).map(|_| item("")).collect();
        assert!(matches!(
            prepare(too_many, [0, 1]),
            Err(FaceSceneError::DocumentBound)
        ));
        let oversized = vec![item(&"a".repeat(MAX_DOCUMENT_BYTES + 1))];
        assert!(matches!(
            prepare(oversized, [0, 1]),
            Err(FaceSceneError::DocumentBound)
        ));
    }
}
