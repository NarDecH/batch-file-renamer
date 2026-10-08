//! Core engine: pure logic, no UI imports. Used by both GUI and CLI.

pub mod executor;
pub mod logger;
pub mod metadata;
pub mod models;
pub mod natsort;
pub mod preview;
pub mod rules;
pub mod scanner;
pub mod undo_store;
