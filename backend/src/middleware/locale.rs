//! Locale extraction middleware
//!
//! Extracts the locale from the Accept-Language header and sets it
//! for the current request context.

use axum::{
    extract::Request,
    http::header::ACCEPT_LANGUAGE,
    middleware::Next,
    response::Response,
};

use crate::utils::{extract_locale_from_header, set_locale};

/// Middleware to extract locale from Accept-Language header
pub async fn locale_middleware(req: Request, next: Next) -> Response {
    // Extract locale from Accept-Language header
    let locale = req
        .headers()
        .get(ACCEPT_LANGUAGE)
        .and_then(|v| v.to_str().ok());
    
    let locale = extract_locale_from_header(locale);
    
    // Set locale for current thread
    set_locale(&locale);
    
    // Store locale in request extensions for handlers to access
    // (Note: extensions are not used here since we use thread-local storage)
    
    next.run(req).await
}
