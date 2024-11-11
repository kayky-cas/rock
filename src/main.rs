use std::{
    fs::File,
    path::{Path, PathBuf},
    sync::Arc,
};

mod config;
mod response;
mod variable;

use clap::Parser;
use tokio::{
    io::{AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use tokio_native_tls::TlsConnector;

const HTTPS_DEFAULT_PORT: u16 = 443;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Arg {
    #[arg(short, long)]
    port: u16,

    #[arg(short, long)]
    file: PathBuf,
}

fn substitute_hostname(buf: &[u8], host: &str) -> Vec<u8> {
    let request_str = String::from_utf8_lossy(buf);

    // TODO: convert this to use regex
    request_str
        .lines()
        .map(|line| {
            if line.starts_with("Host:") {
                format!("Host: {}\r\n", host)
            } else {
                format!("{}\r\n", line)
            }
        })
        .collect::<String>()
        .into_bytes()
}

async fn proxy<C, S>(mut client: C, mut server: S, buf: &[u8]) -> anyhow::Result<()>
where
    C: AsyncWrite + Unpin,
    S: AsyncWriteExt + AsyncReadExt + Unpin,
{
    server.write_all(buf).await?;
    tokio::io::copy(&mut server, &mut client).await?;
    Ok(())
}

async fn redirect(
    client: TcpStream,
    path: &str,
    method: config::ConfigMethod,
    proxy_addr: &config::ProxyAddr,
    buf: &[u8],
) -> anyhow::Result<()> {
    println!(
        "[PROXY] {} {}:{}{}",
        method,
        proxy_addr.host(),
        proxy_addr.port(),
        path
    );

    let buf = substitute_hostname(buf, proxy_addr.host());
    let server = TcpStream::connect(proxy_addr.to_tuple()).await?;

    if proxy_addr.port() == HTTPS_DEFAULT_PORT {
        let connector = TlsConnector::from(native_tls::TlsConnector::new()?);
        let server = connector.connect(proxy_addr.host(), server).await?;
        proxy(client, server, &buf).await
    } else {
        proxy(client, server, &buf).await
    }
}

async fn accept(mut stream: TcpStream, file_path: Arc<Path>) -> anyhow::Result<()> {
    let mut buf = [0; 1024 * 4];

    let n = stream.read(&mut buf).await?;
    let content = &buf[..n];

    let mut visitor = content.split(|b| *b == b' ');

    let method = visitor
        .next()
        .ok_or_else(|| anyhow::anyhow!("missing method"))?
        .try_into()?;

    let path = String::from_utf8_lossy(
        visitor
            .next()
            .ok_or_else(|| anyhow::anyhow!("missing path"))?,
    );

    let file = File::open(file_path)?;
    let config: config::Config = serde_json::from_reader(file)?;

    let Some(response) = config
        .responses()
        .iter()
        .filter(|response| response.is_valid(method))
        .find_map(|request| {
            let variables = variable::PathVariables::new(request.path());
            let variables_table = variable::extract_variables(&variables, &path).ok()?;

            response::Response::try_new(request, variables_table).ok()
        })
    else {
        return redirect(stream, &path, method, config.proxy_addr(), content).await;
    };

    println!("[MOCK]  {} {}", method, path);
    let _ = stream.write(response.as_http().as_bytes()).await?;

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let arg = Arg::parse();
    let listener = TcpListener::bind(("0.0.0.0", arg.port)).await?;

    let file_path: Arc<Path> = arg.file.into();

    println!("Rocking on :{}", arg.port);

    loop {
        let Ok((stream, _)) = listener.accept().await else {
            continue;
        };

        let file_path = file_path.clone();
        tokio::spawn(async {
            if let Err(err) = accept(stream, file_path).await {
                eprintln!("[ERROR] {}", err);
            }
        });
    }
}
