//! Internationalization utilities for the backend
//!
//! This module provides locale extraction from HTTP requests and
//! thread-local storage for the current locale.

use std::cell::RefCell;

// Thread-local storage for current locale
thread_local! {
    static CURRENT_LOCALE: RefCell<String> = RefCell::new("zh".to_string());
}

/// Supported locales
pub const SUPPORTED_LOCALES: &[&str] = &["zh", "en"];
pub const DEFAULT_LOCALE: &str = "zh";

/// Set the current locale for the current thread
pub fn set_locale(locale: &str) {
    let locale = normalize_locale(locale);
    CURRENT_LOCALE.with(|l| {
        *l.borrow_mut() = locale;
    });
}

/// Get the current locale for the current thread
pub fn get_locale() -> String {
    CURRENT_LOCALE.with(|l| l.borrow().clone())
}

/// Normalize locale string to supported format
/// Accepts: "zh", "zh-CN", "zh_CN", "en", "en-US", "en_US", etc.
fn normalize_locale(locale: &str) -> String {
    let locale = locale.trim().to_lowercase();
    
    // Extract primary language tag
    let primary = locale
        .split(|c| c == '-' || c == '_' || c == ',')
        .next()
        .unwrap_or(DEFAULT_LOCALE);
    
    // Map to supported locale
    if primary.starts_with("zh") {
        "zh".to_string()
    } else if primary.starts_with("en") {
        "en".to_string()
    } else {
        DEFAULT_LOCALE.to_string()
    }
}

/// Extract locale from Accept-Language header value
pub fn extract_locale_from_header(header_value: Option<&str>) -> String {
    match header_value {
        Some(value) => normalize_locale(value),
        None => DEFAULT_LOCALE.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_locale() {
        assert_eq!(normalize_locale("zh"), "zh");
        assert_eq!(normalize_locale("zh-CN"), "zh");
        assert_eq!(normalize_locale("zh_CN"), "zh");
        assert_eq!(normalize_locale("en"), "en");
        assert_eq!(normalize_locale("en-US"), "en");
        assert_eq!(normalize_locale("en_US"), "en");
        assert_eq!(normalize_locale("fr"), "zh"); // Unsupported, fallback to default
        assert_eq!(normalize_locale(""), "zh");
    }

    #[test]
    fn test_set_get_locale() {
        set_locale("en");
        assert_eq!(get_locale(), "en");
        
        set_locale("zh-CN");
        assert_eq!(get_locale(), "zh");
    }
}
