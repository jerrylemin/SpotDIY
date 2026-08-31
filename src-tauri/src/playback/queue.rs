//! Playback-owned queue surface.
//!
//! The implementation remains in the Plan 03 queue module for compatibility
//! with existing callers; this module is the playback boundary and re-exports
//! the same transient types without duplicating queue state.
pub use crate::queue::{QueueEntry, QueueEntryId, RepeatMode, TransientQueue};
