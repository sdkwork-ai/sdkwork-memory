//! SDKWork Memory search-first vector runtime plugin.
//!
//! This crate provides the mem0-style search-first memory capability as a
//! provider-neutral SDKWork Memory runtime plugin. It exports exactly two SPI
//! ports, [`MemoryRetrieverPort`](sdkwork_memory_spi::MemoryRetrieverPort) and
//! [`MemoryIndexPort`](sdkwork_memory_spi::MemoryIndexPort), and consumes every
//! provider through an injected port.
//!
//! # Boundaries
//!
//! * **No canonical ownership.** `ai_record` and `ai_event` stay the source of
//!   truth; this plugin's vector index is derived and rebuildable.
//! * **No provider clients.** There is no HTTP client, endpoint literal, secret
//!   read, or environment lookup in this crate.
//! * **No silent degradation.** Every path that cannot honour a request reports
//!   it through `degraded` and `degradation_codes`, or fails closed.
//!
//! # Where the mem0 behaviour lives
//!
//! | mem0 behaviour | Module |
//! | --- | --- |
//! | Vector similarity search | [`vector_index`] |
//! | ADD-only extraction from turns | [`extraction`] |
//! | Procedural summarisation and the agent-context suffix | [`procedural`] |
//! | Content-hash deduplication and link resolution | [`additive`] |
//! | Provider wiring | [`runtime`] |
//!
//! # Write path
//!
//! The write path is additive, matching mem0 V3: [`extraction::extract_memories`]
//! turns conversation turns into candidate memories, and
//! [`additive::plan_additions`] decides which of them to commit. There is no
//! `UPDATE`, `DELETE`, or `NOOP` event, because the reference pipeline has none.
//!
//! The one exception is the procedural path, which the reference routes away from
//! additive extraction entirely: [`procedural::resolve_write_path`] decides which
//! of the two a request takes, and every other memory type is refused rather than
//! routed into either.
//!
//! `src/lib.rs` deliberately contains only module declarations and re-exports.

#![warn(missing_docs)]

pub mod additive;
pub mod config;
pub mod error;
pub mod extraction;
pub mod manifest;
pub mod procedural;
pub mod runtime;
pub mod vector_index;

pub use additive::*;
pub use config::*;
pub use error::*;
pub use extraction::*;
pub use manifest::*;
pub use procedural::*;
pub use runtime::*;
pub use vector_index::*;
