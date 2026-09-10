use std::env;
use std::path::PathBuf;
use std::sync::Arc;

use lxs_daemon::server::DaemonServer;
use lxs_runtime::Runtime;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn socket_path() -> PathBuf {
    env::var("LXS_SOCKET_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            env::var("XDG_RUNTIME_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("/tmp"))
                .join("linux-x11-sandbox.sock")
        })
}

fn pid_path() -> PathBuf {
    env::var("LXS_PID_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let mut p = socket_path();
            p.set_extension("pid");
            p
        })
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    let command = args.get(1).map(|s| s.as_str()).unwrap_or("mcp");

    match command {
        "serve" => run_serve().await,
        "mcp" => run_mcp().await,
        "stop" => run_stop().await,
        "status" => run_status().await,
        _ => {
            eprintln!("Usage: linux-x11-sandbox [serve|mcp|stop|status]");
            std::process::exit(1);
        }
    }
}

async fn run_serve() {
    let socket_path = socket_path();
    let pid = std::process::id();
    let _ = tokio::fs::write(pid_path(), pid.to_string()).await;

    let runtime = Arc::new(Runtime::new());
    let server = DaemonServer::new(runtime, socket_path);
    if let Err(e) = server.run().await {
        eprintln!("daemon error: {e}");
        std::process::exit(1);
    }
}

async fn run_stop() {
    let pid_path = pid_path();
    let pid: u32 = match tokio::fs::read_to_string(&pid_path).await {
        Ok(s) => s.trim().parse().unwrap_or(0),
        Err(_) => {
            eprintln!("daemon not running");
            std::process::exit(1);
        }
    };
    if pid == 0 {
        eprintln!("daemon not running");
        std::process::exit(1);
    }
    unsafe {
        libc::kill(pid as i32, libc::SIGTERM);
    }
    let _ = tokio::fs::remove_file(&pid_path).await;
    let socket_path = socket_path();
    let _ = tokio::fs::remove_file(&socket_path).await;
    println!("stopped daemon {pid}");
}

async fn run_status() {
    if !socket_path().exists() {
        println!("stopped");
        std::process::exit(1);
    }
    println!("running");
}

async fn run_mcp() {
    let socket_path = socket_path();
    if !socket_path.exists() {
        let executable = env::current_exe().expect("current exe");
        let mut child = tokio::process::Command::new(executable)
            .arg("serve")
            .spawn()
            .expect("spawn daemon");

        for _ in 0..50 {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            if socket_path.exists() {
                break;
            }
        }

        if !socket_path.exists() {
            let _ = child.kill().await;
            eprintln!("daemon failed to start");
            std::process::exit(1);
        }
    }

    let stream = tokio::net::UnixStream::connect(&socket_path)
        .await
        .expect("connect daemon");

    let (read_half, mut write_half) = stream.into_split();
    let stdin_to_socket = forward(tokio::io::stdin(), &mut write_half);
    let socket_to_stdout = forward(read_half, tokio::io::stdout());

    stdin_to_socket.await;
    let _ = write_half.shutdown().await;
    socket_to_stdout.await;
}

async fn forward<R, W>(mut reader: R, mut writer: W)
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    let mut buf = [0u8; 8192];
    loop {
        match reader.read(&mut buf).await {
            Ok(0) => break,
            Ok(n) => {
                if writer.write_all(&buf[..n]).await.is_err() {
                    break;
                }
                if writer.flush().await.is_err() {
                    break;
                }
            }
            Err(_) => break,
        }
    }
}
