//! Closed wire choices for an authenticated component Mask selection.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BrowserComponentLook {
    Compact,
    Spacious,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BrowserComponentRecipe {
    Flat,
    Grouped,
    #[serde(rename = "required-item")]
    RequiredItem,
}
