use std::{
    fs::File,
    path::{Path, PathBuf},
    sync::Arc,
};

mod config;
mod response;
mod variable;

use anyhow::Context;
use clap::Parser;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

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
    let r = regex::Regex::new(r"Host: ([^\r\n]+)").expect("should be a valid regex");

    r.replace_all(&request_str, format!("Host: {}", host))
        .to_string()
        .into_bytes()
}

async fn proxy<W, R>(
    mut client: TcpStream,
    mut writer: W,
    mut reader: R,
    buf: &[u8],
) -> anyhow::Result<()>
where
    W: AsyncWriteExt + Unpin,
    R: AsyncReadExt + Unpin,
{
    let (c, w) = tokio::join!(
        tokio::io::copy(&mut reader, &mut client),
        writer.write_all(buf),
    );

    c.context("failed to copy from server to client")?;
    w.context("failed to copy from client to server")
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

    if proxy_addr.port() == 443 {
        let connector = tokio_native_tls::TlsConnector::from(native_tls::TlsConnector::new()?);

        let stream = connector
            .connect(proxy_addr.host(), server)
            .await
            .context("failed to connect to server")?;

        let (r, w) = tokio::io::split(stream);
        proxy(client, w, r, &buf).await
    } else {
        let (r, w) = server.into_split();
        proxy(client, w, r, &buf).await
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

    println!("[MOCKS] {} {}", method, path);
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
