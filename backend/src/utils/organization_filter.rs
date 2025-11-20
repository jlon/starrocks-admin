/// Apply organization filter to a base SQL query.
/// Returns (filtered_sql, needs_org_bind) tuple.
/// If `is_super_admin` is true, returns original query with no bind needed.
/// Otherwise, appends organization filter; if org_id is None, adds impossible condition.
pub fn apply_organization_filter(
    base_query: &str,
    is_super_admin: bool,
    org_id: Option<i64>,
) -> (String, bool) {
    if is_super_admin {
        return (base_query.to_string(), false);
    }

    let (query_without_order, order_clause) = split_order_clause(base_query);

    if let Some(org) = org_id {
        let filtered =
            append_condition(&query_without_order, &format!("organization_id = {}", org));
        (format!("{}{}", filtered, order_clause), false)
    } else {
        let filtered = append_condition(&query_without_order, "1 = 0");
        (format!("{}{}", filtered, order_clause), false)
    }
}

fn split_order_clause(query: &str) -> (String, String) {
    let uppercase = query.to_uppercase();
    if let Some(idx) = uppercase.find(" ORDER BY ") {
        let (head, tail) = query.split_at(idx);
        (head.trim_end().to_string(), tail.to_string())
    } else {
        (query.to_string(), String::new())
    }
}

fn append_condition(query: &str, condition: &str) -> String {
    if query.to_uppercase().contains(" WHERE ") {
        format!("{} AND {}", query, condition)
    } else {
        format!("{} WHERE {}", query, condition)
    }
}
