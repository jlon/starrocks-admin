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

pub use client::LLMClient;
pub use models::*;
pub use repository::LLMRepository;
pub use scenarios::merger::*;
pub use scenarios::root_cause::*;
pub use service::{LLMAnalysisRequestTrait, LLMAnalysisResponseTrait, LLMService, LLMServiceImpl};

#[cfg(test)]
mod tests;
