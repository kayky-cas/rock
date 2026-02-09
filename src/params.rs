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
