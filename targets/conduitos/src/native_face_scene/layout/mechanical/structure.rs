//! Borrowed checked topology and bottom-up selected intrinsic demands.
use super::*;
#[derive(Clone, Copy, Default)]
pub(super) struct Demand {
    pub minimum: u16,
    pub preferred: u16,
    pub inline_grow: u8,
}
pub(super) struct Group<'a> {
    pub children: &'a [ComponentLayoutChild],
    pub row: bool,
    pub inline: &'a Alignment,
    pub block: &'a Alignment,
    pub breaks: &'a Break,
    pub padding: u16,
    pub gap: u16,
    pub inline_grow: u8,
    pub inline_shrink: u8,
}
pub(super) enum Node<'a> {
    Leaf(Option<usize>),
    Group(Group<'a>),
}
pub(super) fn node<'a>(
    context: &Context<'_>,
    node: &'a ComponentLayoutNode,
) -> Result<Node<'a>, FaceSceneError> {
    macro_rules! group {
        ($g:expr, $row:expr) => {{
            let g = $g;
            if *g.inline_shrink() > 1 {
                return Err(FaceSceneError::UnsupportedStructure);
            }
            Node::Group(Group {
                children: g.children().as_slice(),
                row: $row,
                inline: g.inline_alignment(),
                block: g.block_alignment(),
                breaks: g.break_policy(),
                padding: context.em(*g.padding_em_sixteenths())?,
                gap: context.em(*g.gap_em_sixteenths())?,
                inline_grow: *g.inline_grow(),
                inline_shrink: *g.inline_shrink(),
            })
        }};
    }
    Ok(match node {
        ComponentLayoutNode::Region(index) => Node::Leaf(Some(usize::from(*index))),
        ComponentLayoutNode::Fallback => Node::Leaf(None),
        ComponentLayoutNode::Row(group) => group!(group, true),
        ComponentLayoutNode::Column(group) => group!(group, false),
        ComponentLayoutNode::Grid(_) => return Err(FaceSceneError::UnsupportedStructure),
    })
}

pub(super) fn measure(
    context: &Context<'_>,
    candidate: &ComponentLayoutCandidate,
    index: usize,
    demands: &mut [Demand; 32],
) -> Result<Demand, FaceSceneError> {
    let result = match node(context, &candidate.nodes().as_slice()[index])? {
        Node::Leaf(region) => {
            let mut result = Demand {
                minimum: 1,
                preferred: 1,
                inline_grow: 1,
            };
            for (_, item) in context.items(region) {
                let extra = u32::from(context.padding) * 2
                    + u32::from(item.indent) * u32::from(INDENT_STEP)
                    + u32::from(super::super::super::semantic_icons::inset(item.icon))
                    + if item.checkbox.is_some() {
                        u32::from(super::super::super::boolean_choice::INSET)
                    } else {
                        0
                    };
                let mut remaining = item.text.as_str();
                while !remaining.is_empty() {
                    let mut end = remaining.len().min(conduit_face::MAX_GRAPHICS_TEXT_BYTES);
                    while !remaining.is_char_boundary(end) {
                        end -= 1;
                    }
                    let metrics = intrinsic_text_metrics(
                        &remaining[..end],
                        context.typography.style(item.role.into()),
                        context.scale,
                    )
                    .map_err(|_| FaceSceneError::InvalidExtent)?;
                    let minimum = u16::try_from(metrics.minimum_inline + extra)
                        .map_err(|_| FaceSceneError::InvalidExtent)?;
                    result.minimum = result.minimum.max(minimum);
                    result.preferred = result.preferred.max(
                        (metrics.preferred_inline + extra).min(u32::from(context.width)) as u16,
                    );
                    remaining = &remaining[end..];
                }
            }
            result.preferred = result.preferred.max(result.minimum);
            result
        }
        Node::Group(group) => {
            let mut minimum = 0;
            let mut preferred = 0u32;
            for child in group.children {
                let child = measure(context, candidate, usize::from(*child.node()), demands)?;
                if group.row {
                    minimum = add(minimum, child.minimum)?;
                    preferred += u32::from(child.preferred);
                } else {
                    minimum = minimum.max(child.minimum);
                    preferred = preferred.max(u32::from(child.preferred));
                }
            }
            let gaps = if group.row {
                group
                    .gap
                    .checked_mul(group.children.len().saturating_sub(1) as u16)
                    .ok_or(FaceSceneError::InvalidExtent)?
            } else {
                0
            };
            let edges = add(group.padding, group.padding)?;
            let minimum = add(add(minimum, gaps)?, edges)?;
            let preferred = preferred + u32::from(gaps) + u32::from(edges);
            if group.inline_shrink == 0 && preferred > u32::from(context.width) {
                return Err(FaceSceneError::InvalidExtent);
            }
            let preferred = (preferred.min(u32::from(context.width)) as u16).max(minimum);
            Demand {
                minimum: if group.inline_shrink == 0 {
                    preferred
                } else {
                    minimum
                },
                preferred,
                inline_grow: group.inline_grow,
            }
        }
    };
    demands[index] = result;
    Ok(result)
}

/// Interpolate admitted minima toward preferred demand, then distribute surplus
/// by explicit inline_grow weights. At most two 32-child passes; remainder follows child
/// order as a visual tie-break only. Maximum is the admitted available width.
pub(super) fn widths(
    group: &Group<'_>,
    available: u16,
    demands: &[Demand; 32],
) -> Result<[u16; 32], FaceSceneError> {
    let mut widths = [0; 32];
    let gaps = group
        .gap
        .checked_mul(group.children.len().saturating_sub(1) as u16)
        .ok_or(FaceSceneError::InvalidExtent)?;
    let available = available
        .checked_sub(gaps)
        .ok_or(FaceSceneError::InvalidExtent)?;
    let mut minimum = 0u32;
    let mut delta = 0u32;
    let mut inline_grow = 0u32;
    for (i, child) in group.children.iter().enumerate() {
        let demand = demands[usize::from(*child.node())];
        widths[i] = demand.minimum;
        minimum += u32::from(demand.minimum);
        delta += u32::from(demand.preferred - demand.minimum);
        inline_grow += u32::from(demand.inline_grow);
    }
    let mut extra = u32::from(available)
        .checked_sub(minimum)
        .ok_or(FaceSceneError::InvalidExtent)?;
    let toward = extra.min(delta);
    let mut remaining_delta = delta;
    let mut remaining_toward = toward;
    for (i, child) in group.children.iter().enumerate() {
        let demand = demands[usize::from(*child.node())];
        let wanted = u32::from(demand.preferred - demand.minimum);
        let allocation = (remaining_toward * wanted)
            .checked_div(remaining_delta)
            .unwrap_or(0);
        widths[i] += allocation as u16;
        remaining_delta -= wanted;
        remaining_toward -= allocation;
    }
    extra -= toward;
    for (i, child) in group.children.iter().enumerate() {
        let weight = u32::from(demands[usize::from(*child.node())].inline_grow);
        let allocation = (extra * weight).checked_div(inline_grow).unwrap_or(0);
        widths[i] += allocation as u16;
        inline_grow -= weight;
        extra -= allocation;
    }
    Ok(widths)
}
