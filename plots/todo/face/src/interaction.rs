//! Translate an exact current Face action into the Todo Plot's typed command.
//! The caller delivers the resulting Form to the admitted Play ingress; this
//! module does not retain state or mutate it outside the scan.

use alloc::{string::String, string::ToString};
use conduit_presentation::{FaceInteraction, MaskShow, Presentation, PresentationPropertyValue};
use conduit_todo_plot::{TodoCommand, TodoState};

use crate::TodoFaceError;

pub fn todo_command_from_interaction(
    state: &TodoState,
    face: &Presentation,
    show: &MaskShow,
    interaction: &FaceInteraction,
) -> Result<TodoCommand, TodoFaceError> {
    state.validate().map_err(|_| TodoFaceError::InvalidState)?;
    if !face.properties.iter().any(|property| {
        property.subject == "todo/list"
            && property.name == "todo-revision"
            && property.value == PresentationPropertyValue::Count(u64::from(state.revision))
    }) {
        return Err(TodoFaceError::StaleState);
    }
    interaction
        .validate_against(face, show)
        .map_err(TodoFaceError::InvalidInteraction)?;
    let command = command_for_action(
        state,
        &interaction.action_id,
        &interaction.target,
        interaction
            .arguments
            .first()
            .map(|argument| argument.value.as_slice()),
    )?;
    state
        .apply(&command)
        .map_err(TodoFaceError::CommandRefused)?;
    Ok(command)
}

fn command_for_action(
    state: &TodoState,
    action: &str,
    target: &str,
    argument: Option<&[u8]>,
) -> Result<TodoCommand, TodoFaceError> {
    if action == "todo.add" && target == "todo/list" {
        let text = core::str::from_utf8(argument.ok_or(TodoFaceError::InvalidActionContract)?)
            .map_err(|_| TodoFaceError::InvalidActionContract)?;
        return Ok(TodoCommand::Add {
            text: text.to_string(),
        });
    }
    if argument.is_some() {
        return Err(TodoFaceError::InvalidActionContract);
    }
    for item in &state.items {
        if target != format_item_target(&item.id) {
            continue;
        }
        let command = if !item.complete && action == format_action("complete", &item.id) {
            TodoCommand::SetComplete {
                id: item.id.clone(),
                complete: true,
            }
        } else if item.complete && action == format_action("reopen", &item.id) {
            TodoCommand::SetComplete {
                id: item.id.clone(),
                complete: false,
            }
        } else if action == format_action("remove", &item.id) {
            TodoCommand::Remove {
                id: item.id.clone(),
            }
        } else {
            return Err(TodoFaceError::UnsupportedAction);
        };
        return Ok(command);
    }
    Err(TodoFaceError::UnsupportedAction)
}

fn format_item_target(id: &str) -> String {
    alloc::format!("todo/item/{id}")
}

fn format_action(verb: &str, id: &str) -> String {
    alloc::format!("todo.{verb}.{id}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_todo_plot::TodoItem;

    fn state() -> TodoState {
        TodoState {
            title: "Groceries".into(),
            revision: 0,
            next_id: 3,
            items: alloc::vec![
                TodoItem {
                    id: "task-1".into(),
                    text: "Milk".into(),
                    complete: false
                },
                TodoItem {
                    id: "task-2".into(),
                    text: "Bread".into(),
                    complete: true
                },
            ],
        }
    }

    #[test]
    fn exact_face_actions_map_to_typed_commands() {
        let state = state();
        assert_eq!(
            command_for_action(&state, "todo.add", "todo/list", Some(b"Tea")),
            Ok(TodoCommand::Add { text: "Tea".into() })
        );
        assert_eq!(
            command_for_action(&state, "todo.complete.task-1", "todo/item/task-1", None),
            Ok(TodoCommand::SetComplete {
                id: "task-1".into(),
                complete: true
            })
        );
        assert_eq!(
            command_for_action(&state, "todo.reopen.task-2", "todo/item/task-2", None),
            Ok(TodoCommand::SetComplete {
                id: "task-2".into(),
                complete: false
            })
        );
        assert_eq!(
            command_for_action(&state, "todo.remove.task-1", "todo/item/task-1", None),
            Ok(TodoCommand::Remove {
                id: "task-1".into()
            })
        );
        assert_eq!(
            command_for_action(&state, "todo.complete.task-2", "todo/item/task-2", None),
            Err(TodoFaceError::UnsupportedAction)
        );
    }
}
