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

pub(crate) struct Response {
    status: usize,
    body: String,
    content_type: ContentType,
}

impl Response {
    pub(crate) fn try_new(
        response: &crate::ConfigResponse,
        variables: variable::VariableMap,
    ) -> anyhow::Result<Self> {
        let mut body = serde_json::to_string(&response.body)?;

        for (name, value) in variables {
            body = body.replace(&format!("{{{name}}}"), value);
        }

        Ok(Self {
            status: response.status,
            body,
            content_type: ContentType::from(&response.body),
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
