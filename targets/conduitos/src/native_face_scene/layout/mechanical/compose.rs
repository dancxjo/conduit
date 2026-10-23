//! Bounded tree arrangement and declared continuation under geometry and cost.
use super::*;

pub(super) fn arrange(
    context: &Context<'_>,
    candidate: &ComponentLayoutCandidate,
    index: usize,
    width: u16,
    height: u16,
    demands: &[Demand; 32],
) -> Result<Vec<Fragment>, FaceSceneError> {
    if demands[index].minimum > width {
        return Err(FaceSceneError::InvalidExtent);
    }
    let Node::Group(group) = structure::node(context, &candidate.nodes().as_slice()[index])? else {
        let Node::Leaf(region) = structure::node(context, &candidate.nodes().as_slice()[index])?
        else {
            unreachable!()
        };
        return leaves::arrange(context, region, width, height);
    };
    if group.children.is_empty() {
        return Ok(Vec::new());
    }
    let inner_width = inset(width, group.padding)?;
    let inner_height = inset(height, group.padding)?;
    let mut result = if group.row {
        row(
            context,
            candidate,
            &group,
            inner_width,
            inner_height,
            demands,
        )?
    } else {
        column(
            context,
            candidate,
            &group,
            inner_width,
            inner_height,
            demands,
        )?
    };
    if matches!(group.breaks, Break::Keep) && result.len() > 1 {
        return Err(FaceSceneError::InvalidExtent);
    }
    if result
        .iter()
        .map(|fragment| fragment.entries.len())
        .sum::<usize>()
        > MAX_ROWS
    {
        return Err(FaceSceneError::DocumentBound);
    }
    for fragment in &mut result {
        if matches!(group.breaks, Break::Keep) {
            for (index, entry) in fragment.entries.iter_mut().enumerate() {
                entry.break_before = index == 0;
            }
        }
        for entry in &mut fragment.entries {
            entry.outer_padding = add(entry.outer_padding, group.padding)?;
        }
        fragment.translate(group.padding, group.padding);
        fragment.height = add(fragment.height, group.padding * 2)?;
    }
    Ok(result)
}

fn column(
    context: &Context<'_>,
    candidate: &ComponentLayoutCandidate,
    group: &Group<'_>,
    width: u16,
    height: u16,
    demands: &[Demand; 32],
) -> Result<Vec<Fragment>, FaceSceneError> {
    let mut pages = Vec::new();
    let mut current = Fragment::empty();
    for child in group.children {
        let index = usize::from(*child.node());
        let demand = demands[index];
        let child_width = if matches!(group.inline, Alignment::Stretch) {
            width
        } else {
            demand.preferred.min(width)
        };
        let x = offset(group.inline, width, child_width);
        let fragments = arrange(context, candidate, index, child_width, height, demands)?;
        for mut fragment in fragments {
            while !fragment.entries.is_empty() {
                let y = add(
                    current.height,
                    if current.entries.is_empty() {
                        0
                    } else {
                        group.gap
                    },
                )?;
                let remaining_height = height.saturating_sub(y);
                let remaining_rows = MAX_PAGE_ROWS - current.entries.len();
                let remaining_cost =
                    conduit_face::MAX_GRAPHICS_COMMANDS - chrome::COMMANDS - 1 - current.cost;
                if let Some(mut prefix) =
                    fragment.take_prefix(remaining_height, remaining_rows, remaining_cost)
                {
                    prefix.translate(x, y);
                    current.height = add(y, prefix.height)?;
                    current.cost += prefix.cost;
                    current.entries.extend(prefix.entries);
                } else if current.entries.is_empty() {
                    return Err(FaceSceneError::InvalidExtent);
                } else {
                    pages.push(current);
                    current = Fragment::empty();
                    continue;
                }
                if !fragment.entries.is_empty() {
                    pages.push(current);
                    current = Fragment::empty();
                }
            }
        }
    }
    if !current.entries.is_empty() {
        pages.push(current);
    }
    Ok(pages)
}

fn row(
    context: &Context<'_>,
    candidate: &ComponentLayoutCandidate,
    group: &Group<'_>,
    width: u16,
    height: u16,
    demands: &[Demand; 32],
) -> Result<Vec<Fragment>, FaceSceneError> {
    let widths = structure::widths(group, width, demands)?;
    let used = widths[..group.children.len()]
        .iter()
        .try_fold(0u16, |a, b| add(a, *b))?;
    let used = add(
        used,
        group
            .gap
            .checked_mul(group.children.len().saturating_sub(1) as u16)
            .ok_or(FaceSceneError::InvalidExtent)?,
    )?;
    let mut x = offset(group.inline, width, used);
    // One bounded list per child; all entries are moved into the final pages.
    let mut lanes = Vec::new();
    for (position, child) in group.children.iter().enumerate() {
        let mut fragments = arrange(
            context,
            candidate,
            usize::from(*child.node()),
            widths[position],
            height,
            demands,
        )?;
        for fragment in &mut fragments {
            fragment.translate(x, 0);
        }
        lanes.push(fragments.into_iter().peekable());
        x = add(x, add(widths[position], group.gap)?)?;
    }
    let mut pages = Vec::new();
    // Every nonempty page consumes at least one safe block. At most MAX_ROWS
    // pages; no retry search or hidden convergence condition.
    loop {
        let mut current = Fragment::empty();
        let mut pending = false;
        for lane in &mut lanes {
            let Some(fragment) = lane.peek_mut() else {
                continue;
            };
            pending = true;
            let remaining_rows = MAX_PAGE_ROWS - current.entries.len();
            let remaining_cost =
                conduit_face::MAX_GRAPHICS_COMMANDS - chrome::COMMANDS - 1 - current.cost;
            if let Some(mut prefix) = fragment.take_prefix(height, remaining_rows, remaining_cost) {
                let marker = prefix.height;
                for entry in &mut prefix.entries {
                    entry.cross_extent = marker;
                }
                current.height = current.height.max(prefix.height);
                current.cost += prefix.cost;
                current.entries.extend(prefix.entries);
            }
            if fragment.entries.is_empty() {
                lane.next();
            }
        }
        if !pending {
            break;
        }
        if current.entries.is_empty() {
            return Err(FaceSceneError::InvalidExtent);
        }
        pages.push(current);
    }
    for page in &mut pages {
        for entry in &mut page.entries {
            entry.row.bounds.y += offset(group.block, page.height, entry.cross_extent) as i16;
            entry.cross_extent = 0;
        }
    }
    Ok(pages)
}
