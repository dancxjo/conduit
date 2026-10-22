/// Canonical actor indices enumerate the prepared inventory, retaining sparse
/// native target slots. A label or a service-returned index grants no selection.
fn selection_target(graphs: &[Option<Box<PatchbayGraph>>; SLOTS], action: &str) -> Option<usize> {
    let suffix = action.strip_prefix("patchbay.plot.")?;
    if suffix.is_empty()
        || (suffix.len() > 1 && suffix.starts_with('0'))
        || !suffix.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    let index: usize = suffix.parse().ok()?;
    graphs
        .iter()
        .enumerate()
        .filter(|(_, graph)| graph.is_some())
        .nth(index)
        .map(|(target, _)| target)
}
