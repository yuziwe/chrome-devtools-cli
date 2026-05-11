use anyhow::{bail, Result};
use std::time::Duration;
#[cfg(windows)]
use tokio::net::TcpStream;
#[cfg(unix)]
use tokio::net::UnixStream;

use crate::protocol::*;

#[cfg(unix)]
async fn connect_daemon() -> Result<UnixStream> {
    Ok(UnixStream::connect(socket_path()).await?)
}

#[cfg(windows)]
async fn connect_daemon() -> Result<TcpStream> {
    let addr = std::fs::read_to_string(addr_path())?;
    Ok(TcpStream::connect(addr.trim()).await?)
}

/// Try to send a request to the daemon. Returns error if daemon is not running.
pub async fn send_to_daemon(request: &DaemonRequest) -> Result<DaemonResponse> {
    let mut stream = connect_daemon().await?;

    let req_bytes = serde_json::to_vec(request)?;
    write_msg(&mut stream, &req_bytes).await?;

    let resp_bytes = read_msg(&mut stream).await?;
    let response: DaemonResponse = serde_json::from_slice(&resp_bytes)?;
    Ok(response)
}

/// Spawn the daemon process in the background.
pub fn spawn_daemon(ws_url: &str) -> Result<()> {
    let exe = std::env::current_exe()?;
    std::process::Command::new(&exe)
        .args(["__daemon__", ws_url])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;
    Ok(())
}

/// Wait for the daemon socket to become available.
pub async fn wait_for_daemon() -> Result<()> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        if tokio::time::Instant::now() > deadline {
            bail!("Daemon failed to start within 5 seconds");
        }
        if connect_daemon().await.is_ok() {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
