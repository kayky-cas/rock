use std::fmt::Display;

use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize, PartialEq, Debug, Clone, Copy)]
#[serde(rename_all = "UPPERCASE")]
pub(crate) enum ConfigMethod {
    Get,
    Post,
    Put,
    Delete,
}

impl TryFrom<&[u8]> for ConfigMethod {
    type Error = anyhow::Error;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        Ok(match value {
            b"GET" => ConfigMethod::Get,
            b"POST" => ConfigMethod::Post,
            b"PUT" => ConfigMethod::Put,
            b"DELETE" => ConfigMethod::Delete,
            method => anyhow::bail!(
                "unsupported HTTP method {}",
                String::from_utf8_lossy(method)
            ),
        })
    }
}

impl Display for ConfigMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            ConfigMethod::Get => "GET",
            ConfigMethod::Post => "POST",
            ConfigMethod::Put => "PUT",
            ConfigMethod::Delete => "DELETE",
        })
    }
}

#[derive(Deserialize, Debug)]
pub(crate) struct ProxyAddr {
    host: String,
    port: u16,
}

impl ProxyAddr {
    pub(crate) fn to_tuple(&self) -> (&str, u16) {
        (self.host.as_str(), self.port)
    }

    pub(crate) fn host(&self) -> &str {
        self.host.as_str()
    }

    pub(crate) fn port(&self) -> u16 {
        self.port
    }
}

#[derive(Deserialize, Debug)]
pub(crate) struct Config {
    #[serde(rename = "proxy")]
    proxy_addr: ProxyAddr,
    delay: Option<u64>,
    responses: Vec<ConfigResponse>,
}

impl Config {
    pub(crate) fn proxy_addr(&self) -> &ProxyAddr {
        &self.proxy_addr
    }

    pub(crate) fn responses(&self) -> &[ConfigResponse] {
        self.responses.as_slice()
    }

    pub(crate) fn delay(&self) -> Option<u64> {
        self.delay
    }
}

#[derive(Deserialize, Debug)]
pub(crate) struct ConfigResponse {
    path: String,
    method: ConfigMethod,
    status: usize,
    body: Value,
    enabled: Option<bool>,
    delay: Option<u64>,
}

impl ConfigResponse {
    pub(crate) fn path(&self) -> &str {
        self.path.as_str()
    }

    pub(crate) fn status(&self) -> usize {
        self.status
    }

    pub(crate) fn body(&self) -> &Value {
        &self.body
    }

    pub(crate) fn delay(&self) -> Option<u64> {
        self.delay
    }

    pub(crate) fn is_valid(&self, method: ConfigMethod) -> bool {
        self.enabled.unwrap_or(true) && self.method == method
    }
}
