//! Operator trigger admission is finite and checked before provider acquisition.
use std::process::Command;
#[test]
fn cadenced_operator_admits_only_ordered_complete_trigger_positions() {
    for (positions, accepted) in [
        (vec![1, 3], true),
        (vec![0, 3], false),
        (vec![3, 1], false),
        (vec![1, 1, 3], false),
        (vec![1, 2], false),
        (vec![1, 4], false),
    ] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_xtask"));
        command.args([
            "--dry-run",
            "prove",
            "pete",
            "std-cognition",
            "--utterance",
            "first",
            "--utterance",
            "second",
            "--utterance",
            "third",
        ]);
        for position in positions {
            command.args(["--reflection-after", &position.to_string()]);
        }
        let output = command.output().unwrap();
        assert_eq!(
            output.status.success(),
            accepted,
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        if accepted {
            assert!(String::from_utf8_lossy(&output.stdout)
                .contains("3 utterances and 2 reflection triggers"));
        } else {
            assert!(String::from_utf8_lossy(&output.stderr).contains("reflection positions"));
        }
    }
}

#[test]
#[ignore = "requires loopback Ollama with gemma3:latest already installed"]
fn live_cadenced_operator_recalls_across_a_replacement_host() {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "conduit-live-cadenced-cli-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir(&root).unwrap();
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let root = Cleanup(root);
    let checkpoint = root.0.join("memory");
    let first_archive = root.0.join("first.jsonl");
    let first_replay = root.0.join("first-replay.json");
    let first = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args([
            "prove",
            "pete",
            "std-cognition",
            "--retain",
            "--checkpoint-dir",
        ])
        .arg(&checkpoint)
        .arg("--episodes-out")
        .arg(&first_archive)
        .arg("--replay-out")
        .arg(&first_replay)
        .args([
            "--utterance",
            "The hallway switch label is COPPER KITE.",
            "--utterance",
            "I passed the hallway.",
            "--utterance",
            "What was the hallway switch label? Reply with the label only.",
            "--reflection-after",
            "1",
            "--reflection-after",
            "3",
        ])
        .output()
        .unwrap();
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let rows = |path: &std::path::Path| {
        std::fs::read_to_string(path)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
            .collect::<Vec<_>>()
    };
    let first_rows = rows(&first_archive);
    assert_eq!(first_rows.len(), 2);
    assert_eq!(first_rows[1]["recalled_episodes"], serde_json::json!([1]));
    assert!(first_rows[1]["outcome"]["content"]
        .as_str()
        .unwrap()
        .contains("COPPER KITE"));
    let second_archive = root.0.join("second.jsonl");
    let second_replay = root.0.join("second-replay.json");
    let second = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(["prove", "pete", "std-cognition", "--checkpoint-dir"])
        .arg(&checkpoint)
        .arg("--episodes-out")
        .arg(&second_archive)
        .arg("--replay-out")
        .arg(&second_replay)
        .args([
            "--utterance",
            "What was the hallway switch label? Reply with the label only.",
            "--reflection-after",
            "1",
        ])
        .output()
        .unwrap();
    assert!(
        second.status.success(),
        "{}",
        String::from_utf8_lossy(&second.stderr)
    );
    let second_rows = rows(&second_archive);
    let resumed = &second_rows[0];
    assert_eq!(resumed["episode"], 3);
    assert_eq!(resumed["body_id"], first_rows[0]["body_id"]);
    assert_eq!(resumed["wake_id"], first_rows[0]["wake_id"]);
    assert_ne!(
        resumed["observation"]["identity"],
        first_rows[0]["observation"]["identity"]
    );
    assert_ne!(resumed["outcome"]["run"], first_rows[0]["outcome"]["run"]);
    assert!(resumed["recalled_episodes"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!(1)));
    assert!(resumed["outcome"]["content"]
        .as_str()
        .unwrap()
        .contains("COPPER KITE"));
    let replay = |path: &std::path::Path| {
        serde_json::from_slice::<serde_json::Value>(&std::fs::read(path).unwrap()).unwrap()
    };
    let first_view = replay(&first_replay);
    let second_view = replay(&second_replay);
    assert_ne!(
        first_view["face"]["basis"]["plan_id"],
        second_view["face"]["basis"]["plan_id"]
    );
    assert_ne!(
        first_view["face"]["basis"]["active_play_id"],
        second_view["face"]["basis"]["active_play_id"]
    );
    assert_eq!(
        first_view["timeline"]["trace"]["missing_parent_sequences"],
        serde_json::json!([])
    );
    println!("Live cadenced operator: episodes 1, 2, 3; one Body/Wake; distinct Plans, Plays, source Signs and model runs; historical label recalled.");
}
