//! Patchbay compatibility names for the shared renderer-neutral application theme.
//!
//! Theme values are presentation decoration. They are deliberately absent
//! from Form, Body, Wake, Plan, Play, Host, Line, Sign, and renderer-plan
//! identities.

pub use conduit_presentation::{ApplicationTheme as PatchbayTheme, ThemeColor};

pub const PHOSPHOR_THEME: PatchbayTheme = conduit_presentation::CONDUIT_APPLICATION_THEME;
