use crate::process::Step;

pub(super) const STEPS: &[Step] = &[
    Step::new(
        "todo-speech.selection",
        "Prove concise openings, exact Show pages, stale selection and audio pressure refusals",
        "cargo",
        &["test", "--locked", "-p", "conduit-std-host", "--lib", "spoken_face_mask"],
    ),
    Step::new(
        "todo-speech.model",
        "Prove finite model grammar and retained refusal of invented or unselected facts/actions",
        "cargo",
        &["test", "--locked", "-p", "conduit-std-host", "--lib", "hosted_local_model::ollama_present"],
    ),
    Step::new(
        "todo-speech.plot",
        "Prove the actual bounded Todo Face supplies title, counts, primary items and truthful actions",
        "cargo",
        &["test", "--locked", "-p", "conduit", "--bin", "conduit", "todo_speech", "--", "--test-threads=1"],
    ),
    Step::new(
        "todo-speech.input",
        "Prove the ordinary screen-free entrance parses explicit detail commands",
        "cargo",
        &["test", "--locked", "-p", "conduit", "--bin", "conduit", "screen_free_birth::input::tests"],
    ),
];
