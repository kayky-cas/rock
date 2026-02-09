use std::collections::HashMap;

use crate::variable;

pub(crate) fn parse_query(full_path: &[u8], query_start: usize) -> HashMap<String, String> {
    if query_start >= full_path.len() {
        return HashMap::new();
    }

    String::from_utf8_lossy(&full_path[query_start + 1..])
        .split('&')
        .filter_map(|pair| {
            let mut parts = pair.splitn(2, '=');
            Some((parts.next()?.to_string(), parts.next()?.to_string()))
        })
        .collect()
}

pub(crate) fn parse_body(content: &[u8]) -> serde_json::Value {
    content
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .and_then(|pos| serde_json::from_slice(&content[pos + 4..]).ok())
        .unwrap_or(serde_json::Value::Null)
}

fn resolve_body_field(body: &serde_json::Value, key: &str) -> Option<String> {
    let mut current = body;
    for part in key.split('.') {
        current = current.get(part)?;
    }
    match current {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Null => None,
        other => Some(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    // parse_query tests

    #[test]
    fn test_parse_query_single_param() {
        let path = b"/users/42?q=hello";
        let query_start = path.iter().position(|&c| c == b'?').unwrap();
        let result = parse_query(path, query_start);
        assert_eq!(result, HashMap::from([("q".into(), "hello".into())]));
    }

    #[test]
    fn test_parse_query_multiple_params() {
        let path = b"/search?q=rust&page=2&sort=desc";
        let query_start = path.iter().position(|&c| c == b'?').unwrap();
        let result = parse_query(path, query_start);
        assert_eq!(
            result,
            HashMap::from([
                ("q".into(), "rust".into()),
                ("page".into(), "2".into()),
                ("sort".into(), "desc".into()),
            ])
        );
    }

    #[test]
    fn test_parse_query_no_query_string() {
        let path = b"/users/42";
        let result = parse_query(path, path.len());
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_query_value_with_equals() {
        let path = b"/test?expr=a=b";
        let query_start = path.iter().position(|&c| c == b'?').unwrap();
        let result = parse_query(path, query_start);
        assert_eq!(result, HashMap::from([("expr".into(), "a=b".into())]));
    }

    // parse_body tests

    #[test]
    fn test_parse_body_valid_json() {
        let content = b"POST /users HTTP/1.1\r\nContent-Type: application/json\r\n\r\n{\"name\":\"kai\"}";
        let result = parse_body(content);
        assert_eq!(result, serde_json::json!({"name": "kai"}));
    }

    #[test]
    fn test_parse_body_nested_json() {
        let content = b"POST / HTTP/1.1\r\n\r\n{\"user\":{\"address\":{\"city\":\"SP\"}}}";
        let result = parse_body(content);
        assert_eq!(result, serde_json::json!({"user": {"address": {"city": "SP"}}}));
    }

    #[test]
    fn test_parse_body_no_body() {
        let content = b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let result = parse_body(content);
        assert_eq!(result, serde_json::Value::Null);
    }

    #[test]
    fn test_parse_body_invalid_json() {
        let content = b"POST / HTTP/1.1\r\n\r\nnot json";
        let result = parse_body(content);
        assert_eq!(result, serde_json::Value::Null);
    }

    #[test]
    fn test_parse_body_no_separator() {
        let content = b"GET / HTTP/1.1";
        let result = parse_body(content);
        assert_eq!(result, serde_json::Value::Null);
    }

    // resolve_body_field tests

    #[test]
    fn test_resolve_body_field_top_level_string() {
        let body = serde_json::json!({"name": "kai"});
        assert_eq!(resolve_body_field(&body, "name"), Some("kai".into()));
    }

    #[test]
    fn test_resolve_body_field_nested() {
        let body = serde_json::json!({"address": {"city": "SP"}});
        assert_eq!(resolve_body_field(&body, "address.city"), Some("SP".into()));
    }

    #[test]
    fn test_resolve_body_field_number() {
        let body = serde_json::json!({"age": 25});
        assert_eq!(resolve_body_field(&body, "age"), Some("25".into()));
    }

    #[test]
    fn test_resolve_body_field_boolean() {
        let body = serde_json::json!({"active": true});
        assert_eq!(resolve_body_field(&body, "active"), Some("true".into()));
    }

    #[test]
    fn test_resolve_body_field_null() {
        let body = serde_json::json!({"value": null});
        assert_eq!(resolve_body_field(&body, "value"), None);
    }

    #[test]
    fn test_resolve_body_field_missing() {
        let body = serde_json::json!({"name": "kai"});
        assert_eq!(resolve_body_field(&body, "missing"), None);
    }

    #[test]
    fn test_resolve_body_field_deeply_nested() {
        let body = serde_json::json!({"a": {"b": {"c": {"d": "deep"}}}});
        assert_eq!(resolve_body_field(&body, "a.b.c.d"), Some("deep".into()));
    }

    // substitute tests

    #[test]
    fn test_substitute_path_vars() {
        let path_vars = HashMap::from([("id", "42")]);
        let query_params = HashMap::new();
        let request_body = serde_json::Value::Null;

        let mut body = r#"{"id":"{/id}"}"#.to_string();
        substitute(&mut body, &path_vars, &query_params, &request_body);
        assert_eq!(body, r#"{"id":"42"}"#);
    }

    #[test]
    fn test_substitute_query_params() {
        let path_vars = HashMap::new();
        let query_params = HashMap::from([("q".into(), "hello".into())]);
        let request_body = serde_json::Value::Null;

        let mut body = r#"{"search":"{?q}"}"#.to_string();
        substitute(&mut body, &path_vars, &query_params, &request_body);
        assert_eq!(body, r#"{"search":"hello"}"#);
    }

    #[test]
    fn test_substitute_body_fields() {
        let path_vars = HashMap::new();
        let query_params = HashMap::new();
        let request_body = serde_json::json!({"username": "kai", "address": {"city": "SP"}});

        let mut body = r#"{"user":"{#username}","city":"{#address.city}"}"#.to_string();
        substitute(&mut body, &path_vars, &query_params, &request_body);
        assert_eq!(body, r#"{"user":"kai","city":"SP"}"#);
    }

    #[test]
    fn test_substitute_all_three_sources() {
        let path_vars = HashMap::from([("id", "42")]);
        let query_params = HashMap::from([("q".into(), "hello".into())]);
        let request_body = serde_json::json!({"username": "kai"});

        let mut body = r#"{"id":"{/id}","search":"{?q}","user":"{#username}"}"#.to_string();
        substitute(&mut body, &path_vars, &query_params, &request_body);
        assert_eq!(body, r#"{"id":"42","search":"hello","user":"kai"}"#);
    }

    #[test]
    fn test_substitute_missing_placeholders_remain() {
        let path_vars = HashMap::new();
        let query_params = HashMap::new();
        let request_body = serde_json::Value::Null;

        let mut body = r#"{"id":"{/id}","q":"{?q}","name":"{#name}"}"#.to_string();
        substitute(&mut body, &path_vars, &query_params, &request_body);
        assert_eq!(body, r#"{"id":"{/id}","q":"{?q}","name":"{#name}"}"#);
    }
}

pub(crate) fn substitute(
    body: &mut String,
    path_vars: &variable::VariableMap,
    query_params: &HashMap<String, String>,
    request_body: &serde_json::Value,
) {
    for (name, value) in path_vars {
        *body = body.replace(&format!("{{/{name}}}"), value);
    }

    for (name, value) in query_params {
        *body = body.replace(&format!("{{?{name}}}"), value);
    }

    while let Some(start) = body.find("{#") {
        let Some(end) = body[start..].find('}') else {
            break;
        };
        let end = start + end;
        let key = &body[start + 2..end];
        if let Some(value) = resolve_body_field(request_body, key) {
            *body = body[..start].to_string() + &value + &body[end + 1..];
        } else {
            break;
        }
    }
}
