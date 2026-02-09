use std::collections::HashMap;
use std::fmt::Display;

use crate::variable;

enum ContentType {
    Json,
    Plain,
}

impl From<&serde_json::Value> for ContentType {
    fn from(value: &serde_json::Value) -> Self {
        if value.is_string() {
            ContentType::Plain
        } else {
            ContentType::Json
        }
    }
}

impl Display for ContentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContentType::Json => f.write_str("application/json"),
            ContentType::Plain => f.write_str("text/plain"),
        }
    }
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

pub(crate) struct Response {
    status: usize,
    body: String,
    content_type: ContentType,
}

impl Response {
    pub(crate) fn try_new(
        response: &crate::config::ConfigResponse,
        path_vars: variable::VariableMap,
        query_params: &HashMap<String, String>,
        request_body: &serde_json::Value,
    ) -> anyhow::Result<Self> {
        let mut body = serde_json::to_string(response.body())?;

        for (name, value) in &path_vars {
            body = body.replace(&format!("{{/{name}}}"), value);
        }

        for (name, value) in query_params {
            body = body.replace(&format!("{{?{name}}}"), value);
        }

        // Replace {#key} and {#key.nested} with values from request body
        while let Some(start) = body.find("{#") {
            let Some(end) = body[start..].find('}') else {
                break;
            };
            let end = start + end;
            let key = &body[start + 2..end];
            if let Some(value) = resolve_body_field(request_body, key) {
                body = body[..start].to_string() + &value + &body[end + 1..];
            } else {
                break;
            }
        }

        Ok(Self {
            status: response.status(),
            body,
            content_type: ContentType::from(response.body()),
        })
    }

    pub(crate) fn as_http(&self) -> String {
        format!(
            "HTTP/1.1 {status}\r\nContent-Type: {content_type}; charset=utf-8\r\nContent-Length: {content_len}\r\n\r\n{content}",
            status = self.status,
            content_type = self.content_type,
            content_len = self.body.len(),
            content = self.body
        )
    }
}
