//! Preparation-only nested structure over checked recipe regions. This profile
//! is not admitted by component-mask@2 and performs no measurement or rendering.
use crate::{
    prepare_component_recipe, ComponentLayoutCandidate, ComponentLayoutChild, ComponentLayoutInfo,
    ComponentLayoutNode, ComponentRecipeRefusal, Face, PreparedComponentRecipe,
};
use conduit_core::StructuredInfoValue;
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};

pub const COMPONENT_LAYOUT_MAX_NODES: usize = 32;
pub const COMPONENT_LAYOUT_MAX_DEPTH: usize = 4;
pub const COMPONENT_LAYOUT_MAX_CANDIDATES: usize = 2;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ComponentLayoutRefusal {
    Encoding,
    Recipe(ComponentRecipeRefusal),
    RecipeIdentity,
    Capacity,
    NodeReference,
    Cycle,
    DuplicateParent,
    UnreachableNode,
    Depth,
    RegionReference,
    DuplicateRegion,
    MissingRegion,
    Fallback,
    Placement,
    GridOverlap,
}

/// Keeps the exact Face immutably borrowed for the entire checked result's
/// lifetime. Reading and action truth remain in that Face and its checked recipe;
/// candidate child order is only visual structure, never semantic/action order.
pub struct PreparedComponentLayout<'a> {
    face: &'a Face,
    recipe: PreparedComponentRecipe,
    structure: ComponentLayoutInfo,
}
impl PreparedComponentLayout<'_> {
    pub fn face(&self) -> &Face {
        self.face
    }
    pub fn recipe(&self) -> &PreparedComponentRecipe {
        &self.recipe
    }
    pub fn structure(&self) -> &ComponentLayoutInfo {
        &self.structure
    }
}

/// Reuses typed slot validation before checking finite visual topology. Every
/// candidate contains every recipe region once, plus one explicit generic leaf
/// for all unmatched valid Face subjects, even when that set is empty.
///
/// At most 32 nodes across two candidates, depth four, and sixteen grid cells
/// per group. Each child edge is inspected once per group then once by DFS;
/// fixed scratch is 32 visitation bytes, eight region flags and sixteen cells.
/// Empty row/column/grid groups are legal; they cannot replace required leaves.
pub fn prepare_component_layout<'a>(
    structure: StructuredInfoValue,
    recipe: StructuredInfoValue,
    face: &'a Face,
) -> Result<PreparedComponentLayout<'a>, ComponentLayoutRefusal> {
    use ComponentLayoutRefusal as R;
    let structure = ComponentLayoutInfo::from_structured(structure).map_err(|_| R::Encoding)?;
    let recipe = prepare_component_recipe(recipe, face).map_err(R::Recipe)?;
    if structure.recipe_identity() != recipe.recipe.identity() {
        return Err(R::RecipeIdentity);
    }
    if structure.candidates().is_empty()
        || structure.candidates().len() > COMPONENT_LAYOUT_MAX_CANDIDATES
        || structure
            .candidates()
            .iter()
            .map(|c| c.nodes().len())
            .sum::<usize>()
            > COMPONENT_LAYOUT_MAX_NODES
    {
        return Err(R::Capacity);
    }
    for candidate in structure.candidates().iter() {
        check_candidate(candidate, recipe.recipe.regions().len())?;
    }
    Ok(PreparedComponentLayout {
        face,
        recipe,
        structure,
    })
}

fn check_candidate(
    candidate: &ComponentLayoutCandidate,
    region_count: usize,
) -> Result<(), ComponentLayoutRefusal> {
    use ComponentLayoutRefusal as R;
    let mut check = CandidateCheck {
        candidate,
        states: [0; COMPONENT_LAYOUT_MAX_NODES],
        regions: [false; 8],
        region_count,
        fallback: false,
    };
    check.visit(usize::from(*candidate.root()), 1)?;
    if check.states[..candidate.nodes().len()].contains(&0) {
        return Err(R::UnreachableNode);
    }
    if check.regions[..region_count].contains(&false) {
        return Err(R::MissingRegion);
    }
    if !check.fallback {
        return Err(R::Fallback);
    }
    Ok(())
}

struct CandidateCheck<'a> {
    candidate: &'a ComponentLayoutCandidate,
    states: [u8; COMPONENT_LAYOUT_MAX_NODES],
    regions: [bool; 8],
    region_count: usize,
    fallback: bool,
}
impl CandidateCheck<'_> {
    fn visit(&mut self, index: usize, depth: usize) -> Result<(), ComponentLayoutRefusal> {
        use ComponentLayoutRefusal as R;
        let node = self
            .candidate
            .nodes()
            .as_slice()
            .get(index)
            .ok_or(R::NodeReference)?;
        match self.states[index] {
            1 => return Err(R::Cycle),
            2 => return Err(R::DuplicateParent),
            _ => (),
        }
        if depth > COMPONENT_LAYOUT_MAX_DEPTH {
            return Err(R::Depth);
        }
        self.states[index] = 1;
        match node {
            ComponentLayoutNode::Region(region) => {
                let region = usize::from(*region);
                if region >= self.region_count {
                    return Err(R::RegionReference);
                }
                if core::mem::replace(&mut self.regions[region], true) {
                    return Err(R::DuplicateRegion);
                }
            }
            ComponentLayoutNode::Fallback => {
                if core::mem::replace(&mut self.fallback, true) {
                    return Err(R::Fallback);
                }
            }
            ComponentLayoutNode::Row(group) => {
                check_placements(group.children(), None)?;
                for child in group.children().iter() {
                    self.visit(usize::from(*child.node()), depth + 1)?;
                }
            }
            ComponentLayoutNode::Column(group) => {
                check_placements(group.children(), None)?;
                for child in group.children().iter() {
                    self.visit(usize::from(*child.node()), depth + 1)?;
                }
            }
            ComponentLayoutNode::Grid(grid) => {
                check_placements(
                    grid.group().children(),
                    Some((*grid.inline_tracks(), *grid.block_tracks())),
                )?;
                for child in grid.group().children().iter() {
                    self.visit(usize::from(*child.node()), depth + 1)?;
                }
            }
        }
        self.states[index] = 2;
        Ok(())
    }
}

fn check_placements(
    children: &BoundedSequence<ComponentLayoutChild, 32>,
    grid: Option<(u8, u8)>,
) -> Result<(), ComponentLayoutRefusal> {
    use ComponentLayoutRefusal as R;
    let mut occupied = [false; 16];
    for child in children.iter() {
        let (inline, block) = (*child.inline_track(), *child.block_track());
        let (inline_span, block_span) = (*child.inline_span(), *child.block_span());
        if let Some((inline_tracks, block_tracks)) = grid {
            let inline_end = inline.checked_add(inline_span).ok_or(R::Placement)?;
            let block_end = block.checked_add(block_span).ok_or(R::Placement)?;
            if inline_end > inline_tracks || block_end > block_tracks {
                return Err(R::Placement);
            }
            for row in block..block_end {
                for col in inline..inline_end {
                    let cell = usize::from(row) * 4 + usize::from(col);
                    if core::mem::replace(&mut occupied[cell], true) {
                        return Err(R::GridOverlap);
                    }
                }
            }
        } else if (inline, block, inline_span, block_span) != (0, 0, 1, 1) {
            // Row/column child order supplies composition. Grid metadata cannot
            // silently acquire meaning outside an explicitly declared grid.
            return Err(R::Placement);
        }
    }
    Ok(())
}
