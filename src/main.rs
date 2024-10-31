use std::fs::File;

use serde::Deserialize;
use serde_json::Value;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
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
struct Proxy {
    uri: String,
    port: u16,
}

#[derive(Deserialize, Debug)]
struct Config {
    proxy: Proxy,
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

async fn redirect(mut stream: TcpStream, addr: &str, port: u16, buf: &[u8]) -> anyhow::Result<()> {
    let request_str = String::from_utf8_lossy(buf);

    let buf = request_str
        .lines()
        .map(|line| {
            if line.starts_with("Host:") {
                format!("Host: {}:{}\r\n", addr, port)
            } else {
                format!("{}\r\n", line)
            }
        })
        .collect::<String>();

    let mut proxy = TcpStream::connect((addr, port)).await?;

    if port == HTTPS_PORT {
        let connector = native_tls::TlsConnector::new()?;
        let connector = TlsConnector::from(connector);
        let mut proxy = connector.connect(addr, proxy).await?;

        let _ = proxy.write_all(buf.as_bytes()).await?;
        let _ = tokio::io::copy(&mut proxy, &mut stream).await?;
    } else {
        let _ = proxy.write_all(buf.as_bytes()).await?;
        let _ = tokio::io::copy(&mut proxy, &mut stream).await?;
    }

    Ok(())
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
            return redirect(
                stream,
                config.proxy.uri.as_str(),
                config.proxy.port,
                content,
            )
            .await;
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
