//! Modeler shell panels (wave A, card UI-A4).
//!
//! Each of the five named regions of the editor shell is rendered by one
//! module here. `app.rs` owns the region layout ([`crate::config::ShellLayout`
//! `::regions`]) and calls into these as the panel seam, so later cards fill a
//! panel without having to touch `app.rs`:
//!
//! - [`tree`]        - model tree (left column)
//! - [`viewport`]    - viewport (centre column); UI-A5 fills the live frame
//! - [`properties`]  - properties panel (right column)
//! - [`status`]      - status bar (bottom strip): document, selection, mode
//! - [`palette`]     - command palette host (top strip); UI-A6 fills it
//! - [`doc`]         - document lifecycle seam (open/save); UI-A7 fills it

pub mod doc;
pub mod palette;
pub mod properties;
pub mod status;
pub mod tree;
pub mod viewport;
