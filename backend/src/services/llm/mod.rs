//! LLM Service Module
//!
//! Provides LLM-enhanced analysis capabilities for StarRocks Admin.
//! LLM is a generic capability - root cause analysis is just one implementation.
//!
//! # Architecture
//! ```text
//! ┌─────────────────┐
//! │   LLMService    │  ← Trait (generic interface)
//! └────────┬────────┘
//!          │
//!    ┌─────┴─────┐
//!    ▼           ▼
//! ┌──────┐  ┌──────────┐
//! │OpenAI│  │ Future   │
//! │Client│  │ Providers│
//! └──────┘  └──────────┘
//! ```
//!
//! # Supported Scenarios
//! - Root Cause Analysis (profile diagnostics)
//! - SQL Optimization (future)
//! - Parameter Tuning (future)
//! - DDL Optimization (future)

mod client;
mod models;
mod repository;
mod scenarios;
mod service;

// Re-exports for external use
pub use models::*;
pub use service::{LLMService, LLMServiceImpl};

// Internal use - exported for specific scenarios
pub use scenarios::root_cause::*;

// These are used internally or for advanced scenarios
pub(crate) use client::LLMClient;
pub(crate) use repository::LLMRepository;
pub(crate) use scenarios::merger::*;
pub(crate) use service::{LLMAnalysisRequestTrait, LLMAnalysisResponseTrait};

#[cfg(test)]
mod tests;
