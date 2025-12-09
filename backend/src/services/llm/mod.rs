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
pub use service::{LLMAnalysisResult, LLMService, LLMServiceImpl};

// Internal use - exported for specific scenarios
pub use scenarios::root_cause::*;

// Allow unused for internal modules (used in tests or future features)
#[allow(unused_imports)]
pub(crate) use client::LLMClient;
#[allow(unused_imports)]
pub(crate) use repository::LLMRepository;
#[allow(unused_imports)]
pub(crate) use scenarios::merger::*;
#[allow(unused_imports)]
pub(crate) use service::{LLMAnalysisRequestTrait, LLMAnalysisResponseTrait};

#[cfg(test)]
mod tests;
