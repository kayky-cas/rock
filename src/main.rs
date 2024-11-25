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
    sync::OnceCell,
};

static HOST_RE: OnceCell<regex::Regex> = OnceCell::const_new();

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Arg {
    #[arg(short, long)]
    port: u16,

    #[arg(short, long)]
    file: PathBuf,

    #[arg(short, long, default_value = "false")]
    show_body: bool,
}

async fn substitute_hostname(buf: &[u8], host: &str) -> Vec<u8> {
    let request_str = String::from_utf8_lossy(buf);

    let buf = HOST_RE
        .get_or_init(|| async { regex::Regex::new(r"Host: ([^\r\n]+)").expect("invalid regex") })
        .await
        .replace_all(&request_str, format!("Host: {}", host));

    match buf {
        std::borrow::Cow::Borrowed(_) => buf.as_bytes().to_vec(),
        std::borrow::Cow::Owned(buf) => buf.into_bytes(),
    }
}

async fn proxy<W, R>(
    client: &mut TcpStream,
    mut writer: W,
    mut reader: R,
    buf: &[u8],
) -> anyhow::Result<()>
where
    W: AsyncWriteExt + Unpin,
    R: AsyncReadExt + Unpin,
{
    tokio::try_join!(tokio::io::copy(&mut reader, client), writer.write_all(buf))
        .context("failed to proxy")?;

    Ok(())
}

async fn redirect(
    client: &mut TcpStream,
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

    let (buf, server) = tokio::join!(
        substitute_hostname(buf, proxy_addr.host()),
        TcpStream::connect(proxy_addr.to_tuple())
    );

    let server = server.context("failed to connect to server")?;

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

async fn accept(stream: &mut TcpStream, file_path: &Path) -> anyhow::Result<()> {
    let mut buf = [0; 1024 * 4];

    let n = stream.read(&mut buf).await?;
    let content = &buf[..n];

    let mut visitor = content.split(|b| *b == b' ');

    let method = visitor.next().context("missing method")?.try_into()?;
    let path = String::from_utf8_lossy(visitor.next().context("missing path")?);

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

    let _ = stream.write(response.as_http().as_bytes()).await?;
    println!("[MOCKS] {} {}", method, path);

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let arg = Arg::parse();
    let listener = TcpListener::bind(("0.0.0.0", arg.port)).await?;

    let file_path: Arc<Path> = arg.file.into();

    println!("Rocking on :{}", arg.port);

    loop {
        let Ok((mut stream, _)) = listener.accept().await else {
            continue;
        };

        let file_path = file_path.clone();
        tokio::spawn(async move {
            if let Err(err) = accept(&mut stream, &file_path).await {
                eprintln!("[ERROR] {}", err);
            }

            if let Err(err) = stream.shutdown().await {
                eprintln!("[ERROR] failed to shutdown stream: {}", err);
            }
        });
    }
}
