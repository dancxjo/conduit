use super::joint;
pub fn source() -> String {
    [
        joint::source(),
        include_str!("../../parser_revision.conduit").into(),
        include_str!("../../parser_session.conduit").into(),
        include_str!("../../parser_session_policy.conduit").into(),
        include_str!("../../parser_session_facts.conduit").into(),
        include_str!("../../parser_session_commit.conduit").into(),
        include_str!("../../parser_session_rebase.conduit").into(),
    ]
    .join("\n")
}
pub fn protected_source() -> String {
    super::protection::append_source(
        [
            joint::runtime_source(),
            include_str!("../../revision_lineage.conduit").into(),
            include_str!("../../parser_available.conduit").into(),
            include_str!("../../parser_revision.conduit").into(),
            include_str!("../../parser_session.conduit").into(),
            include_str!("../../parser_session_facts.conduit").into(),
            include_str!("../../parser_session_rebase.conduit").into(),
        ]
        .join("\n"),
    )
}
