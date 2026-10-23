//! Four-track finite grid negotiation. Single-track contributions precede
//! two-track spans; each deficit is shared equally, remainder in track order.
//! No implicit placement or convergence solver. Span-connected block bands stay
//! together; an oversized band/cell explicitly refuses to the generic reader.
use super::fragment::BlockExtent;
use super::*;

pub(super) fn intrinsic(
    group: &Group<'_>,
    count: u8,
    demands: &[Demand; 32],
) -> Result<[Demand; 4], FaceSceneError> {
    let mut minimum = [0u32; 4];
    let mut preferred = [0u32; 4];
    for span in 1..=2 {
        for child in group
            .children
            .iter()
            .filter(|child| *child.inline_span() == span)
        {
            let demand = demands[usize::from(*child.node())];
            contribute(
                &mut minimum,
                usize::from(*child.inline_track()),
                usize::from(span),
                u32::from(demand.minimum),
                u32::from(group.gap),
            );
        }
    }
    preferred.copy_from_slice(&minimum);
    for span in 1..=2 {
        for child in group
            .children
            .iter()
            .filter(|child| *child.inline_span() == span)
        {
            let demand = demands[usize::from(*child.node())];
            contribute(
                &mut preferred,
                usize::from(*child.inline_track()),
                usize::from(span),
                u32::from(demand.preferred),
                u32::from(group.gap),
            );
        }
    }
    let mut tracks = [Demand::default(); 4];
    for track in 0..usize::from(count) {
        tracks[track].minimum =
            u16::try_from(minimum[track]).map_err(|_| FaceSceneError::InvalidExtent)?;
        tracks[track].preferred =
            u16::try_from(preferred[track]).map_err(|_| FaceSceneError::InvalidExtent)?;
    }
    for child in group.children {
        let grow = demands[usize::from(*child.node())].inline_grow;
        for track in &mut tracks[usize::from(*child.inline_track())
            ..usize::from(*child.inline_track() + *child.inline_span())]
        {
            track.inline_grow = track.inline_grow.max(grow);
        }
    }
    Ok(tracks)
}

fn contribute(tracks: &mut [u32; 4], start: usize, span: usize, required: u32, gap: u32) {
    let current = tracks[start..start + span].iter().sum::<u32>() + gap * (span - 1) as u32;
    let deficit = required.saturating_sub(current);
    let each = deficit / span as u32;
    let remainder = deficit % span as u32;
    for (offset, track) in tracks[start..start + span].iter_mut().enumerate() {
        *track += each + u32::from((offset as u32) < remainder);
    }
}
struct Cell {
    start: usize,
    span: usize,
    x: u16,
    fragment: Fragment,
}

pub(super) fn arrange(
    context: &Context<'_>,
    candidate: &ComponentLayoutCandidate,
    group: &Group<'_>,
    counts: (u8, u8),
    width: u16,
    height: u16,
    demands: &[Demand; 32],
) -> Result<Vec<Fragment>, FaceSceneError> {
    let (inline_count, block_count) = (usize::from(counts.0), usize::from(counts.1));
    let tracks = intrinsic(group, counts.0, demands)?;
    let widths = structure::allocate(&tracks[..inline_count], width, group.gap)?;
    let used = widths[..inline_count]
        .iter()
        .map(|value| u32::from(*value))
        .sum::<u32>()
        + u32::from(group.gap) * (inline_count - 1) as u32;
    let mut origins = [0u16; 4];
    origins[0] = offset(group.inline, width, used as u16);
    for track in 1..inline_count {
        origins[track] = add(origins[track - 1], add(widths[track - 1], group.gap)?)?;
    }
    let mut cells = Vec::new();
    let mut block_tracks = [0u32; 4];
    let mut boundary = [true; 5];
    for child in group.children {
        let start = usize::from(*child.inline_track());
        let span = usize::from(*child.inline_span());
        let available = widths[start..start + span]
            .iter()
            .try_fold(0u16, |sum, width| add(sum, *width))?;
        let available = add(available, group.gap * (span - 1) as u16)?;
        let demand = demands[usize::from(*child.node())];
        let child_width = if matches!(group.inline, Alignment::Stretch) {
            available
        } else {
            available.min(demand.preferred)
        };
        let mut fragments = super::compose::arrange(
            context,
            candidate,
            usize::from(*child.node()),
            child_width,
            height,
            demands,
        )?;
        if fragments.len() > 1 {
            return Err(FaceSceneError::UnsupportedStructure);
        }
        let start = usize::from(*child.block_track());
        let span = usize::from(*child.block_span());
        if span == 2 {
            boundary[start + 1] = false;
        }
        if let Some(fragment) = fragments.pop() {
            cells.push(Cell {
                start,
                span,
                x: add(
                    origins[usize::from(*child.inline_track())],
                    offset(group.inline, available, child_width),
                )?,
                fragment,
            });
        }
    }
    for span in 1..=2 {
        for cell in cells.iter().filter(|cell| cell.span == span) {
            contribute(
                &mut block_tracks,
                cell.start,
                span,
                u32::from(cell.fragment.height),
                u32::from(group.gap),
            );
        }
    }
    let mut pages = Vec::new();
    let mut current = Fragment::empty();
    let mut start = 0;
    let mut page_origin = 0u32;
    let mut last_marker = 0;
    while start < block_count {
        let end = (start + 1..=block_count)
            .find(|index| boundary[*index])
            .unwrap_or(block_count);
        let selected: Vec<_> = cells
            .iter()
            .enumerate()
            .filter(|(_, cell)| cell.start >= start && cell.start < end)
            .map(|(index, _)| index)
            .collect();
        if !selected.is_empty() {
            let band_height = block_tracks[start..end].iter().sum::<u32>()
                + u32::from(group.gap) * (end - start - 1) as u32;
            if band_height > u32::from(height) {
                return Err(FaceSceneError::UnsupportedStructure);
            }
            let mut band = Fragment::empty();
            band.height = band_height as u16;
            for index in selected {
                let cell = &mut cells[index];
                let y = block_tracks[start..cell.start].iter().sum::<u32>()
                    + u32::from(group.gap) * (cell.start - start) as u32;
                let available = block_tracks[cell.start..cell.start + cell.span]
                    .iter()
                    .sum::<u32>()
                    + u32::from(group.gap) * (cell.span - 1) as u32;
                let y = add(
                    y as u16,
                    offset(group.block, available as u16, cell.fragment.height),
                )?;
                cell.fragment.translate(cell.x, y);
                band.cost += cell.fragment.cost;
                band.entries.append(&mut cell.fragment.entries);
            }
            if !band.fits(band.height, 0, 0, height) {
                return Err(FaceSceneError::SceneBound);
            }
            for (index, entry) in band.entries.iter_mut().enumerate() {
                entry.break_before = index == 0;
            }
            band.entries[0].extent = Some(BlockExtent {
                origin: 0,
                end: band.height,
                inset: 0,
            });
            let origin =
                block_tracks[..start].iter().sum::<u32>() + u32::from(group.gap) * start as u32;
            let mut y = origin.saturating_sub(page_origin);
            let target = y + band_height;
            if target > u32::from(height)
                || !current.fits(target as u16, band.entries.len(), band.cost, height)
            {
                if current.entries.is_empty() {
                    return Err(FaceSceneError::UnsupportedStructure);
                }
                pages.push(current);
                current = Fragment::empty();
                page_origin = origin;
                y = 0;
            }
            band.translate(0, y as u16);
            current.height = add(y as u16, band.height)?;
            current.cost += band.cost;
            last_marker = current.entries.len();
            current.entries.extend(band.entries);
        }
        start = end;
    }
    if !current.entries.is_empty() {
        let total = block_tracks[..block_count].iter().sum::<u32>()
            + u32::from(group.gap) * (block_count - 1) as u32;
        let end = total - page_origin;
        if end > u32::from(height) {
            return Err(FaceSceneError::UnsupportedStructure);
        }
        current.height = end as u16;
        current
            .entries
            .get_mut(last_marker)
            .and_then(|entry| entry.extent.as_mut())
            .ok_or(FaceSceneError::InvalidExtent)?
            .end = end as u16;
        pages.push(current);
    }
    Ok(pages)
}
