use std::{collections::HashMap, fmt::Display, fs::File};

use clap::Parser;
use regex::Regex;
use serde::Deserialize;
use serde_json::Value;
use tokio::{
    io::{AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use tokio_native_tls::TlsConnector;

const HTTPS_PORT: u16 = 443;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Arg {
    #[arg(short, long)]
    port: u16,
}

#[derive(Deserialize, Debug, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
enum Method {
    Get,
    Post,
    Put,
    Delete,
}

impl Display for Method {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Method::Get => "GET",
            Method::Post => "POST",
            Method::Put => "PUT",
            Method::Delete => "DELETE",
        })
    }
}

#[derive(Deserialize, Debug)]
struct ProxyAddr {
    host: String,
    port: u16,
}

impl ProxyAddr {
    fn to_tuple(&self) -> (&str, u16) {
        (self.host.as_str(), self.port)
    }
}

#[derive(Deserialize, Debug)]
struct Config {
    #[serde(rename = "proxy")]
    proxy_addr: ProxyAddr,
    responses: Vec<Response>,
}

#[derive(Deserialize, Debug)]
struct Response {
    path: String,
    method: Method,
    status: usize,
    body: Value,
    enabled: Option<bool>,
}

fn into_http(status: usize, body: &str) -> String {
    format!(
        "HTTP/1.1 {}\r\nContent-Type: text/json; charset=utf-8\r\nContent-Length: {}\r\n\r\n{}",
        status,
        body.len(),
        body
    )
}

fn substitute_hostname(buf: &[u8], proxy_addr: &ProxyAddr) -> Vec<u8> {
    let request_str = String::from_utf8_lossy(buf);

    request_str
        .lines()
        .flat_map(|line| {
            if line.starts_with("Host:") {
                format!("Host: {}:{}\r\n", proxy_addr.host, proxy_addr.port)
            } else {
                format!("{}\r\n", line)
            }
            .into_bytes()
        })
        .collect()
}

async fn proxy<C, S>(mut client: C, mut server: S, buf: &[u8]) -> anyhow::Result<()>
where
    C: AsyncWrite + Unpin,
    S: AsyncWriteExt + AsyncReadExt + Unpin,
{
    server.write_all(buf).await?;
    let _ = tokio::io::copy(&mut server, &mut client).await?;

    Ok(())
}

async fn redirect(
    client: TcpStream,
    path: &str,
    method: Method,
    proxy_addr: ProxyAddr,
    buf: &[u8],
) -> anyhow::Result<()> {
    println!(
        "[PROXY] {} {}:{}{}",
        method, proxy_addr.host, proxy_addr.port, path
    );

    let buf = substitute_hostname(buf, &proxy_addr);

    let server = TcpStream::connect(proxy_addr.to_tuple()).await?;

    if proxy_addr.port == HTTPS_PORT {
        let connector = native_tls::TlsConnector::new()?;
        let connector = TlsConnector::from(connector);
        let server = connector.connect(&proxy_addr.host, server).await?;
        proxy(client, server, &buf).await
    } else {
        proxy(client, server, &buf).await
    }
}

async fn accept(mut stream: TcpStream) -> anyhow::Result<()> {
    let mut buf = [0; 2048];

    let n = stream.read(&mut buf[..]).await?;
    let content = &buf[..n];

    let mut visitor = content.split(|b| *b == b' ');

    let method: Method = match visitor
        .next()
        .ok_or_else(|| anyhow::anyhow!("should have a method"))?
    {
        b"GET" => Method::Get,
        b"POST" => Method::Post,
        b"PUT" => Method::Put,
        b"DELETE" => Method::Delete,
        method => todo!("{:?}", method),
    };

    let path = String::from_utf8_lossy(
        visitor
            .next()
            .ok_or_else(|| anyhow::anyhow!("should have a path"))?,
    );

    let file = File::open("./interface.json")?;
    let config: Config = serde_json::from_reader(file)?;

    let responses = config.responses;

    let (response, variables) = match responses
        .iter()
        .filter_map(|request| {
            if !request.enabled.unwrap_or(true) || request.method != method {
                return None;
            }
            extract_variables(&request.path, &path)
                .ok()
                .map(|variables| (request, variables))
        })
        .next()
    {
        Some(r) => r,
        None => {
            return redirect(stream, &path, method, config.proxy_addr, content).await;
        }
    };

    println!("[MOCK]  {} {}", method, path);

    let mut body = serde_json::to_string(&response.body)?;

    for (name, value) in variables {
        body = body.replace(&format!("{{{name}}}"), value);
    }

    let proto = into_http(response.status, &body);

    let _ = stream.write(proto.as_bytes()).await?;

    Ok(())
}

fn extract_variables<'a, 'b>(
    mut source: &'a str,
    path: &'b str,
) -> anyhow::Result<HashMap<&'a str, &'b str>> {
    if source.starts_with('/') {
        source = &source[1..];
    }

    if source.ends_with('/') {
        source = &source[..source.len() - 1];
    }

    let mut map = HashMap::new();

    let mut pattern = String::from("^");
    let mut variable_names = Vec::new();

    for segment in source.split('/') {
        if segment.starts_with('{') && segment.ends_with('}') {
            let variable_name = &segment[1..segment.len() - 1];
            variable_names.push(variable_name);
            pattern.push_str("/([^/]+)");
        } else {
            pattern.push('/');
            pattern.push_str(segment);
        }
    }
    pattern.push('$');

    let r = Regex::new(&pattern)?;

    if !r.is_match(path) {
        return Err(anyhow::anyhow!("source and path do not match"));
    }

    if let Some(captures) = r.captures(path) {
        for (i, &var_name) in variable_names.iter().enumerate() {
            if let Some(value) = captures.get(i + 1) {
                map.insert(var_name, value.as_str());
            }
        }
    }

    if map.len() != variable_names.len() {
        Err(anyhow::anyhow!("variables do not match"))
    } else {
        Ok(map)
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let arg = Arg::parse();
    let listener = TcpListener::bind(("0.0.0.0", arg.port)).await?;

    println!("Rocking on :{}", arg.port);

    loop {
        let Ok((stream, _)) = listener.accept().await else {
            continue;
        };

        tokio::spawn(accept(stream));
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::extract_variables;

    #[test]
    fn test_extract_variables_should_successes() {
        let src = "/foo/{a}/{b}/baz";
        let path = "/foo/hello/world/baz";

        let expected = HashMap::from([("a", "hello"), ("b", "world")]);

        assert_eq!(extract_variables(src, path).unwrap(), expected)
    }

    #[test]
    fn test_extract_variables_should_fail() {
        let src = "/foo/{a}/{b}/baz";
        let path = "/foo/hello";

        assert!(extract_variables(src, path).is_err())
    }
}
