/// Permission extraction module for cleaner code organization
/// Uses strategy pattern to handle different route patterns
/// Extract permission from URI and method
pub fn extract_permission(method: &str, uri: &str) -> Option<(String, String)> {
    // Routes that don't require permission check (only require authentication)
    // These are basic read-only operations that all authenticated users should access
    if uri == "/api/auth/permissions" {
        return None;
    }
    // GET /api/clusters/active - Get current active cluster (basic info for all users)
    if uri == "/api/clusters/active" && method == "GET" {
        return None;
    }

    let path = uri.strip_prefix("/api/").unwrap_or(uri);
    let segments: Vec<&str> = path.split('/').collect();

    let resource = match *(segments.first()?) {
        "roles" => "roles",
        "permissions" => "permissions",
        "users" => "users",
        "clusters" => "clusters",
        _ => return None,
    };

    // Try special extractors first, then fall back to default
    let action = extract_action_with_special_handlers(resource, &segments, method)
        .or_else(|| extract_action_default(resource, &segments, method))?;

    Some((resource.to_string(), action))
}

/// Extract action with special route handlers
fn extract_action_with_special_handlers(
    resource: &str,
    segments: &[&str],
    method: &str,
) -> Option<String> {
    match resource {
        "roles" => extract_roles_action(segments, method),
        "users" => extract_users_action(segments, method),
        "clusters" => extract_clusters_action_special(segments, method),
        _ => None,
    }
}

/// Extract action for roles resource
fn extract_roles_action(segments: &[&str], method: &str) -> Option<String> {
    if segments.len() >= 3 && segments.get(2) == Some(&"permissions") {
        return match method {
            "PUT" => Some("permissions:update".to_string()),
            "GET" => Some("permissions:get".to_string()),
            _ => None,
        };
    }
    None
}

/// Extract action for users resource
fn extract_users_action(segments: &[&str], method: &str) -> Option<String> {
    if segments.len() >= 3 && segments.get(2) == Some(&"roles") {
        return match method {
            "POST" => Some("roles:assign".to_string()),
            "DELETE" => Some("roles:remove".to_string()),
            "GET" => Some("roles:get".to_string()),
            _ => None,
        };
    }
    None
}

/// Extract action for clusters resource with special handlers
fn extract_clusters_action_special(segments: &[&str], method: &str) -> Option<String> {
    if segments.len() < 2 {
        return None;
    }

    let second = *segments.get(1)?;

    // Check if second segment is an ID
    if second.parse::<i64>().is_ok() {
        return extract_clusters_id_action(segments, method);
    }

    // Special handlers for non-ID paths
    extract_clusters_special_paths(segments, method)
}

/// Extract action for clusters/{id} paths
fn extract_clusters_id_action(segments: &[&str], method: &str) -> Option<String> {
    match segments.len() {
        2 => match method {
            "GET" => Some("get".to_string()),
            "PUT" => Some("update".to_string()),
            "DELETE" => Some("delete".to_string()),
            _ => None,
        },
        _ if segments.len() >= 3 => {
            let action = segments.get(2)?;
            // POST /api/clusters/:id/health -> health:post
            if method == "POST" && *action == "health" {
                Some("health:post".to_string())
            // POST /api/clusters/:id/sql/diagnose -> sql:diagnose
            } else if method == "POST" && *action == "sql" && segments.get(3) == Some(&"diagnose") {
                Some("sql:diagnose".to_string())
            } else {
                Some(action.to_string())
            }
        },
        _ => None,
    }
}

/// Extract action for special cluster paths
type RouteHandler = Box<dyn Fn(&[&str], &str) -> Option<String>>;

fn extract_clusters_special_paths(segments: &[&str], method: &str) -> Option<String> {
    let _second = segments.get(1)?;
    let _len = segments.len();

    // Special route handlers
    let handlers: Vec<RouteHandler> = vec![
        Box::new(|seg, m| {
            if m == "DELETE" && seg.len() == 4 && seg.get(1) == Some(&"backends") {
                Some("backends:delete".to_string())
            } else {
                None
            }
        }),
        // DELETE /api/clusters/queries/:query_id -> queries:kill
        // query_id can be any string (number or UUID with colons)
        // Note: query_id with colons will be split into multiple segments by split('/')
        // Exclude special paths: /queries/history, /queries/execute
        Box::new(|seg, m| {
            if m == "DELETE" && seg.len() >= 3 && seg.get(1) == Some(&"queries") {
                // Check if second segment is a special path (history, execute)
                if let Some(second) = seg.get(2)
                    && (*second == "history" || *second == "execute")
                {
                    return None;
                }
                // Path pattern: /clusters/queries/{query_id}
                // query_id can be any string, not just numbers
                // When query_id contains colons, it will be split into multiple segments
                Some("queries:kill".to_string())
            } else {
                None
            }
        }),
        // GET /api/clusters/queries/:query_id/profile -> queries:profile
        // query_id can be any string (number or UUID with colons)
        // Note: query_id with colons will be split into multiple segments by split('/')
        Box::new(|seg, m| {
            if m == "GET" && seg.len() >= 4 && seg.get(1) == Some(&"queries") {
                // Path pattern: /clusters/queries/{query_id}/profile
                // Check if last segment is "profile" (query_id may contain colons and be split)
                // Exclude special paths like /queries/history/profile (unlikely but safe)
                if let Some(last) = seg.last()
                    && *last == "profile"
                {
                    // Check if second segment is not a special path
                    if let Some(second) = seg.get(2)
                        && (*second == "history" || *second == "execute")
                    {
                        return None;
                    }
                    return Some("queries:profile".to_string());
                }
                None
            } else {
                None
            }
        }),
        // GET /api/clusters/profiles/:query_id -> profiles:get
        // query_id can be any string (number or UUID with colons)
        // Note: query_id with colons will be split into multiple segments by split('/')
        Box::new(|seg, m| {
            if m == "GET" && seg.len() >= 3 && seg.get(1) == Some(&"profiles") {
                // Path pattern: /clusters/profiles/{query_id}
                // query_id can be any string (e.g., UUID with colons like "4ce1242e:bab7:11f0:8a21:9eb34e998e27")
                // When query_id contains colons, it will be split into multiple segments, so we check len >= 3
                Some("profiles:get".to_string())
            } else {
                None
            }
        }),
        // DELETE /api/clusters/sessions/:session_id -> sessions:kill
        // session_id can be any string (number or UUID with colons)
        // Note: session_id with colons will be split into multiple segments by split('/')
        Box::new(|seg, m| {
            if m == "DELETE" && seg.len() >= 3 && seg.get(1) == Some(&"sessions") {
                // Path pattern: /clusters/sessions/{session_id}
                // session_id can be any string, not just numbers
                // When session_id contains colons, it will be split into multiple segments
                Some("sessions:kill".to_string())
            } else {
                None
            }
        }),
        Box::new(extract_materialized_views_action),
        Box::new(extract_variables_action),
        Box::new(extract_system_functions_action),
        Box::new(extract_sql_blacklist_action),
    ];

    for handler in handlers {
        if let Some(action) = handler(segments, method) {
            return Some(action);
        }
    }

    None
}

/// Extract action for materialized_views paths
fn extract_materialized_views_action(segments: &[&str], method: &str) -> Option<String> {
    if segments.get(1) != Some(&"materialized_views") || segments.len() < 3 {
        return None;
    }

    match segments.len() {
        3 => match method {
            "GET" => Some("materialized_views:get".to_string()),
            "PUT" => Some("materialized_views:update".to_string()),
            "DELETE" => Some("materialized_views:delete".to_string()),
            _ => None,
        },
        4 => {
            let action = segments.get(3)?;
            match (*action, method) {
                ("ddl", "GET") => Some("materialized_views:ddl".to_string()),
                ("refresh", "POST") => Some("materialized_views:refresh".to_string()),
                ("cancel", "POST") => Some("materialized_views:cancel".to_string()),
                _ => None,
            }
        },
        _ => None,
    }
}

/// Extract action for variables paths
fn extract_variables_action(segments: &[&str], method: &str) -> Option<String> {
    if method == "PUT" && segments.len() == 3 && segments.get(1) == Some(&"variables") {
        segments.get(2).and_then(|third| {
            if third.parse::<i64>().is_err() { Some("variables:update".to_string()) } else { None }
        })
    } else {
        None
    }
}

/// Extract action for system-functions paths
fn extract_system_functions_action(segments: &[&str], method: &str) -> Option<String> {
    if segments.get(1) != Some(&"system-functions") || segments.len() < 3 {
        return None;
    }

    let third = segments.get(2)?;
    if third.parse::<i64>().is_err() {
        return None;
    }

    match segments.len() {
        3 => match method {
            "PUT" => Some("system:functions:update".to_string()),
            "DELETE" => Some("system:functions:delete".to_string()),
            _ => None,
        },
        4 => {
            let fourth = segments.get(3)?;
            match (*fourth, method) {
                ("execute", "POST") => Some("system:functions:execute".to_string()),
                ("favorite", "PUT") => Some("system:functions:favorite".to_string()),
                _ => None,
            }
        },
        _ => None,
    }
}

/// Extract action for sql-blacklist paths
fn extract_sql_blacklist_action(segments: &[&str], method: &str) -> Option<String> {
    // Handle /api/clusters/sql-blacklist and /api/clusters/sql-blacklist/:id
    if segments.len() >= 2 && segments.get(1) == Some(&"sql-blacklist") {
        match segments.len() {
            2 => match method {
                "GET" => Some("sql:blacklist".to_string()),
                "POST" => Some("sql:blacklist:add".to_string()),
                _ => None,
            },
            3 => match method {
                "DELETE" => Some("sql:blacklist:delete".to_string()),
                _ => None,
            },
            _ => None,
        }
    } else {
        None
    }
}

/// Default action extraction for general cases
/// This handles clusters non-ID paths and other generic routes
fn extract_action_default(resource: &str, segments: &[&str], method: &str) -> Option<String> {
    // For clusters resource, handle non-ID paths by joining segments and normalizing hyphens
    if resource == "clusters" && segments.len() >= 2 {
        let action_parts: Vec<&str> = segments.iter().skip(1).copied().collect();
        let action_str = action_parts.join(":").replace("-", ":");
        return Some(action_str);
    }

    // For other resources, use standard logic
    if segments.len() >= 2 {
        let second = segments.get(1).copied();

        // Check if second segment is a numeric ID
        if let Some(second_str) = second
            && second_str.parse::<i64>().is_ok()
        {
            return match method {
                "GET" => Some("get".to_string()),
                "PUT" => Some("update".to_string()),
                "DELETE" => Some("delete".to_string()),
                _ => None,
            };
        }

        // Non-ID path
        match method {
            "GET" => second.map(|s| s.to_string()),
            _ => None,
        }
    } else {
        // Root path
        match method {
            "GET" => Some("list".to_string()),
            "POST" => Some("create".to_string()),
            _ => None,
        }
    }
}
