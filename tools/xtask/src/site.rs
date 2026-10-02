//! Shared public-site presentation, independent of product runtime state.

pub(crate) fn navigation(current: &str) -> String {
    include_str!("../../../site/navigation.html").replace(
        &format!("data-section=\"{current}\""),
        &format!("data-section=\"{current}\" aria-current=\"page\""),
    )
}

pub(crate) fn styles() -> String {
    format!(
        "{}\n{}",
        include_str!("../../../targets/browser/host/assets/conduit.css"),
        include_str!("../../../site/chrome.css"),
    )
}
