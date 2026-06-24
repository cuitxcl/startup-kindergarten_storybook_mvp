//! Loco/SeaORM model modules live here.
//!
//! In a full Loco app these entity files are typically generated from the
//! database schema with `cargo loco db entities`.
//! The migration crate in `server/migration` is the source of truth for the
//! schema defined from `docs/数据模型设计.md`.
pub mod _entities;
pub mod storybooks;
