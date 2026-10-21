//! Bounded native reading and action navigation over the shared application view.
//! Only five borrowed rows are rendered per page; no copied text corpus or state model.
use conduit_face::{
    ApplicationAction, ApplicationComponent, ApplicationEventKind, ApplicationNodeState,
    ApplicationView, ApplicationViewNode,
};

pub const PAGE_ROWS: usize = 5;
const LINE_CHARACTERS: usize = 40;

#[derive(Clone, Copy, Default)]
pub struct ApplicationViewport {
    pub page: usize,
    pub selected_node: Option<usize>,
}

pub struct Row<'a> {
    pub node_index: usize,
    pub node: &'a ApplicationViewNode,
    pub text: &'a str,
}

/// Visit a finite validated view without allocating line buffers. Labels and
/// values both remain readable; byte and character boundaries are preserved.
pub fn rows<'a>(view: &'a ApplicationView, mut visit: impl FnMut(usize, Row<'a>)) -> usize {
    let mut index = 0;
    for (node_index, node) in view.nodes.iter().enumerate() {
        if node.component == ApplicationComponent::PatchbayCanvas {
            continue;
        }
        for value in [node.text.as_str(), node.value.as_str()] {
            let mut remaining = value;
            while !remaining.is_empty() {
                let mut end = remaining.len();
                for (count, (byte, character)) in remaining.char_indices().enumerate() {
                    if count == LINE_CHARACTERS
                        || byte + character.len_utf8() > conduit_face::MAX_GRAPHICS_TEXT_BYTES
                    {
                        end = byte;
                        break;
                    }
                    if character == '\n' {
                        end = byte + 1;
                        break;
                    }
                }
                // Prefer a word boundary, preserving all consumed characters.
                if end < remaining.len()
                    && !remaining[..end].ends_with('\n')
                    && let Some(space) = remaining[..end].rfind(' ')
                    && space > end / 2
                {
                    end = space + 1;
                }
                visit(
                    index,
                    Row {
                        node_index,
                        node,
                        text: remaining[..end].trim_end(),
                    },
                );
                index += 1;
                remaining = &remaining[end..];
            }
        }
    }
    index
}

pub fn enabled_action<'a>(
    view: &'a ApplicationView,
    node: &ApplicationViewNode,
) -> Option<&'a ApplicationAction> {
    (node.component == ApplicationComponent::Button && node.state == ApplicationNodeState::Ready)
        .then(|| {
            node.action
                .and_then(|index| view.actions.get(usize::from(index)))
        })
        .flatten()
        .filter(|action| action.event == ApplicationEventKind::Activate)
}

impl ApplicationViewport {
    pub fn navigate(
        &mut self,
        view: &ApplicationView,
        usage: u8,
    ) -> Result<bool, crate::data::Refusal> {
        view.validate().map_err(|_| crate::data::Refusal::Layout)?;
        if !matches!(usage, 75 | 78 | 81 | 82) {
            return Ok(false);
        }
        let total = rows(view, |_, _| {});
        let last_page = total.saturating_sub(1) / PAGE_ROWS;
        if matches!(usage, 75 | 78) {
            self.page = if usage == 75 {
                self.page.saturating_sub(1)
            } else {
                self.page.saturating_add(1).min(last_page)
            };
            self.selected_node = None;
        } else {
            let selected = self.selected_node;
            let eligible = |(index, node): &(usize, &ApplicationViewNode)| {
                enabled_action(view, node).is_some()
                    && selected.is_none_or(|current| {
                        if usage == 81 {
                            *index > current
                        } else {
                            *index < current
                        }
                    })
            };
            let next = if usage == 81 {
                view.nodes.iter().enumerate().find(eligible).or_else(|| {
                    view.nodes
                        .iter()
                        .enumerate()
                        .find(|(_, node)| enabled_action(view, node).is_some())
                })
            } else {
                view.nodes
                    .iter()
                    .enumerate()
                    .rev()
                    .find(eligible)
                    .or_else(|| {
                        view.nodes
                            .iter()
                            .enumerate()
                            .rev()
                            .find(|(_, node)| enabled_action(view, node).is_some())
                    })
            };
            if let Some((node_index, _)) = next {
                self.selected_node = Some(node_index);
                let mut first = None;
                rows(view, |index, row| {
                    if row.node_index == node_index && first.is_none() {
                        first = Some(index);
                    }
                });
                if let Some(index) = first {
                    self.page = index / PAGE_ROWS;
                }
            }
        }
        Ok(true)
    }
}
