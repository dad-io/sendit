//! UI rendering modules using trait-based decomposition.
//!
//! Each module defines a trait that `SendItApp` implements,
//! keeping rendering logic separated by concern while sharing app state.

pub mod drop_zone;
pub mod help;
pub mod messages;
pub mod publish;
pub mod query;
pub mod settings;
pub mod topic_tree;
