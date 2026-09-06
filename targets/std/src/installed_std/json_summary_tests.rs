use super::collection_tests::execute_entry;

#[test]
fn todo_summary_counts_remaining_and_completed_after_kernel_edits() {
    for (request, expected) in [
        (
            r#"{"collection":[],"command":{"op":"append","value":{"complete":false,"text":"Buy milk"}}}"#,
            r#"{"false":1,"total":1,"true":0}"#,
        ),
        (
            r#"{"collection":[{"complete":false,"text":"Buy milk"}],"command":{"field":"complete","index":0,"op":"toggle"}}"#,
            r#"{"false":0,"total":1,"true":1}"#,
        ),
        (
            r#"{"collection":[{"complete":true,"text":"Buy milk"}],"command":{"index":0,"op":"remove"}}"#,
            r#"{"false":0,"total":0,"true":0}"#,
        ),
    ] {
        let (output, report) = execute_entry(request, "todo/command-summary");
        report.unwrap();
        assert!(output.lines().any(|line| line == expected), "{output}");
    }
}

#[test]
fn todo_summary_refuses_missing_or_non_boolean_completion() {
    for (value, detail) in [
        (r#"{"text":"Missing completion"}"#, 123),
        (r#"{"complete":0,"text":"Wrong completion"}"#, 124),
    ] {
        let request =
            format!("{{\"collection\":[],\"command\":{{\"op\":\"append\",\"value\":{value}}}}}");
        let (output, report) = execute_entry(&request, "todo/command-summary");
        assert_eq!(
            report.unwrap_err(),
            format!("installed kernel step: OperationFailed({detail})")
        );
        assert!(
            !output.lines().any(|line| line.starts_with('{')),
            "{output}"
        );
    }
}

#[test]
fn todo_restore_decodes_the_actual_edited_snapshot_before_counting() {
    let (output, report) = execute_entry(
        r#"{"collection":[],"command":{"op":"append","value":{"complete":false,"text":"Buy milk"}}}"#,
        "todo/command-snapshot",
    );
    report.unwrap();
    let snapshot = output.lines().find(|line| line.starts_with('[')).unwrap();
    let (restored, report) = execute_entry(snapshot, "todo/restore-summary");
    report.unwrap();
    assert!(restored
        .lines()
        .any(|line| line == r#"{"false":1,"total":1,"true":0}"#));
}

#[test]
fn todo_restore_refuses_corrupt_or_invalid_snapshot_content() {
    for snapshot in [
        "[",
        r#"[{"text":"missing completion"}]"#,
        r#"[{"complete":0}]"#,
    ] {
        let (output, report) = execute_entry(snapshot, "todo/restore-summary");
        assert!(report.is_err(), "{snapshot}");
        assert!(
            !output.lines().any(|line| line.starts_with('{')),
            "{output}"
        );
    }
}
