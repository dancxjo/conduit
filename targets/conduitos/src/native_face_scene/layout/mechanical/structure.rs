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
    pub minimum_inline: u16,
}
pub(super) enum Node<'a> {
    Leaf(Option<usize>),
    Group(Group<'a>),
    Grid(Group<'a>, u8, u8),
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
            Group {
                children: g.children().as_slice(),
                row: $row,
                inline: g.inline_alignment(),
                block: g.block_alignment(),
                breaks: g.break_policy(),
                padding: context.em(*g.padding_em_sixteenths())?,
                gap: context.em(*g.gap_em_sixteenths())?,
                inline_grow: *g.inline_grow(),
                inline_shrink: *g.inline_shrink(),
                minimum_inline: context.em(*g.minimum_inline_em_sixteenths())?,
            }
        }};
    }
    Ok(match node {
        ComponentLayoutNode::Region(index) => Node::Leaf(Some(usize::from(*index))),
        ComponentLayoutNode::Fallback => Node::Leaf(None),
        ComponentLayoutNode::Row(group) => Node::Group(group!(group, true)),
        ComponentLayoutNode::Column(group) => Node::Group(group!(group, false)),
        ComponentLayoutNode::Grid(grid) => Node::Grid(
            group!(grid.group(), false),
            *grid.inline_tracks(),
            *grid.block_tracks(),
        ),
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
                minimum: 0,
                preferred: 0,
                inline_grow: 0,
            };
            for (_, item) in context.items(region) {
                if item.text.is_empty() {
                    continue;
                }
                result.inline_grow = 1;
                let extra = u32::from(context.padding) * 2
                    + u32::from(item.indent) * u32::from(INDENT_STEP)
                    + u32::from(super::super::super::semantic_icons::inset(item.icon))
                    + if item.checkbox.is_some() {
                        u32::from(super::super::super::boolean_choice::INSET)
                    } else {
                        0
                    };
                let metrics = intrinsic_item_text_metrics(
                    item.text.as_str(),
                    context.typography.style(item.role.into()),
                    context.scale,
                    MAX_DOCUMENT_BYTES,
                )
                .map_err(|_| FaceSceneError::InvalidExtent)?;
                let minimum = metrics
                    .minimum_inline
                    .checked_add(extra)
                    .and_then(|value| u16::try_from(value).ok())
                    .ok_or(FaceSceneError::InvalidExtent)?;
                result.minimum = result.minimum.max(minimum);
                let preferred = metrics
                    .preferred_inline
                    .checked_add(extra)
                    .ok_or(FaceSceneError::InvalidExtent)?;
                result.preferred = result
                    .preferred
                    .max(preferred.min(u32::from(context.width)) as u16);
            }
            result.preferred = result.preferred.max(result.minimum);
            result
        }
        Node::Group(group) => {
            let mut minimum = 0u32;
            let mut preferred = 0u32;
            for child in group.children {
                let child = measure(context, candidate, usize::from(*child.node()), demands)?;
                if group.row {
                    minimum += u32::from(child.minimum);
                    preferred += u32::from(child.preferred);
                } else {
                    minimum = minimum.max(u32::from(child.minimum));
                    preferred = preferred.max(u32::from(child.preferred));
                }
            }
            let gaps = if group.row {
                u32::from(group.gap) * group.children.len().saturating_sub(1) as u32
            } else {
                0
            };
            finish_demand(context, &group, minimum + gaps, preferred + gaps)?
        }
        Node::Grid(group, inline_tracks, _) => {
            for child in group.children {
                measure(context, candidate, usize::from(*child.node()), demands)?;
            }
            let tracks = super::grid::intrinsic(&group, inline_tracks, demands)?;
            let gaps = u32::from(group.gap) * u32::from(inline_tracks - 1);
            finish_demand(
                context,
                &group,
                tracks[..usize::from(inline_tracks)]
                    .iter()
                    .map(|d| u32::from(d.minimum))
                    .sum::<u32>()
                    + gaps,
                tracks[..usize::from(inline_tracks)]
                    .iter()
                    .map(|d| u32::from(d.preferred))
                    .sum::<u32>()
                    + gaps,
            )?
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
    let mut children = [Demand::default(); 32];
    for (index, child) in group.children.iter().enumerate() {
        children[index] = demands[usize::from(*child.node())];
    }
    allocate(&children[..group.children.len()], available, group.gap)
}

pub(super) fn allocate(
    demands: &[Demand],
    available: u16,
    gap: u16,
) -> Result<[u16; 32], FaceSceneError> {
    let mut widths = [0; 32];
    let gaps = gap
        .checked_mul(demands.len().saturating_sub(1) as u16)
        .ok_or(FaceSceneError::InvalidExtent)?;
    let available = available
        .checked_sub(gaps)
        .ok_or(FaceSceneError::InvalidExtent)?;
    let mut minimum = 0u32;
    let mut delta = 0u32;
    let mut inline_grow = 0u32;
    for (i, demand) in demands.iter().enumerate() {
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
    for (i, demand) in demands.iter().enumerate() {
        let wanted = u32::from(demand.preferred - demand.minimum);
        let allocation = (remaining_toward * wanted)
            .checked_div(remaining_delta)
            .unwrap_or(0);
        widths[i] += allocation as u16;
        remaining_delta -= wanted;
        remaining_toward -= allocation;
    }
    extra -= toward;
    for (i, demand) in demands.iter().enumerate() {
        let weight = u32::from(demand.inline_grow);
        let allocation = (extra * weight).checked_div(inline_grow).unwrap_or(0);
        widths[i] += allocation as u16;
        inline_grow -= weight;
        extra -= allocation;
    }
    Ok(widths)
}

fn finish_demand(
    context: &Context<'_>,
    group: &Group<'_>,
    minimum: u32,
    preferred: u32,
) -> Result<Demand, FaceSceneError> {
    let edges = u32::from(group.padding) * 2;
    let minimum = (minimum + edges).max(u32::from(group.minimum_inline));
    let preferred = preferred + edges;
    if minimum > u32::from(context.width)
        || (group.inline_shrink == 0 && preferred > u32::from(context.width))
    {
        return Err(FaceSceneError::InvalidExtent);
    }
    let preferred = preferred.min(u32::from(context.width)).max(minimum) as u16;
    Ok(Demand {
        minimum: if group.inline_shrink == 0 {
            preferred
        } else {
            minimum as u16
        },
        preferred,
        inline_grow: group.inline_grow,
    })
}
