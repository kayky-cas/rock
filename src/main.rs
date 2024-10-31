use std::fs::File;

use serde::Deserialize;
use serde_json::Value;
use tokio::{
    io::{AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use tokio_native_tls::TlsConnector;

const HTTPS_PORT: u16 = 443;

#[derive(Deserialize, Debug, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
enum Method {
    Get,
    Post,
    Put,
    Delete,
}

#[derive(Deserialize, Debug)]
struct ProxyAddr {
    host: String,
    port: u16,
}

impl ProxyAddr {
    fn to_tupple(&self) -> (&str, u16) {
        (self.host.as_str(), self.port)
    }
}

#[derive(Deserialize, Debug)]
struct Config {
    proxy_addr: ProxyAddr,
    responses: Vec<Response>,
}

#[derive(Deserialize, Debug)]
struct Response {
    path: String,
    method: Method,
    status: u16,
    body: Value,
    enabled: Option<bool>,
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
    let _ = server.write_all(&buf).await?;
    let _ = tokio::io::copy(&mut server, &mut client).await?;

    Ok(())
}

async fn redirect(client: TcpStream, proxy_addr: ProxyAddr, buf: &[u8]) -> anyhow::Result<()> {
    let buf = substitute_hostname(buf, &proxy_addr);

    let server = TcpStream::connect(proxy_addr.to_tupple()).await?;

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
        _ => todo!(),
    };

    let path: &[u8] = visitor
        .next()
        .ok_or_else(|| anyhow::anyhow!("should have a path"))?;

    let file = File::open("./interface.json").unwrap();
    let config: Config = serde_json::from_reader(file)?;

    let responses = config.responses;

    let response = match responses.iter().find(|request| {
        request.enabled.unwrap_or(true)
            && request.path.as_bytes() == path
            && request.method == method
    }) {
        Some(response) => response,
        None => {
            return redirect(stream, config.proxy_addr, content).await;
        }
    };

    let body = serde_json::to_string(&response.body).unwrap();

    let _ = stream
        .write(
            format!(
                "HTTP/1.1 {}\r\nContent-Type: text/json; charset=utf-8\r\nContent-Length: {}\r\n\r\n{}",
                response.status,
                body.len(),
                body
            )
            .as_bytes(),
        )
        .await?;

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let listener = TcpListener::bind("0.0.0.0:3000").await?;

    loop {
        let Ok((stream, _)) = listener.accept().await else {
            continue;
        };

        tokio::spawn(async { dbg!(accept(stream).await) });
    }
}
