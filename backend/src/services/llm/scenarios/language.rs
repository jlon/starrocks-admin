//! Language-aware helpers for LLM prompts
//!
//! This module centralizes how we decide the natural language that
//! the LLM should use in its responses, based on the current backend
//! locale (thread-local, set by middleware from `Accept-Language`).

/// Get current logical language code used by LLM prompts.
///
/// This maps backend locales into a small set of language tags that
/// are stable for prompt wording and cache keys.
pub fn current_llm_language() -> String {
    // NOTE: We reuse the global i18n locale here so that:
    // - Frontend `Accept-Language` → middleware → thread-local locale
    // - LLM prompts can follow the same language without extra wiring
    let locale = crate::utils::get_locale();
    match locale.as_str() {
        "en" => "en".to_string(),
        // Fallback: treat any other / unknown locale as Simplified Chinese
        _ => "zh".to_string(),
    }
}

/// Build a small prompt section that *hard constrains* the answer language.
///
/// This is intentionally short and strongly worded so that the model
/// does not mix languages in natural language fields.
pub fn build_language_prompt_section() -> String {
    match current_llm_language().as_str() {
        "en" => {
            "\n\n## Language Requirement\n\
Please respond **strictly in English**. Do not use Chinese in any part of the answer.\n"
                .to_string()
        },
        // Default: Simplified Chinese
        _ => {
            "\n\n## 语言要求\n\
请全程使用**简体中文**回答，不要在任何部分使用英文。\n"
                .to_string()
        },
    }
}


