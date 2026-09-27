#[cfg(unix)]
use anyhow::Context;
use anyhow::Result;
use serde::Deserialize;
#[cfg(unix)]
use std::path::PathBuf;
#[cfg(unix)]
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
#[cfg(unix)]
use tokio::net::UnixStream;
#[cfg(unix)]
use tracing::{debug, error, info};

/// Hyprland's command socket path
#[cfg(unix)]
fn hyprland_socket() -> Result<PathBuf> {
    let instance = std::env::var("HYPRLAND_INSTANCE_SIGNATURE")
        .context("HYPRLAND_INSTANCE_SIGNATURE not set — is Hyprland running?")?;
    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/run/user/1000".to_string());
    Ok(PathBuf::from(runtime).join("hypr").join(&instance).join(".socket.sock"))
}

/// Hyprland's event socket path
#[cfg(unix)]
fn hyprland_event_socket() -> Result<PathBuf> {
    let instance = std::env::var("HYPRLAND_INSTANCE_SIGNATURE")
        .context("HYPRLAND_INSTANCE_SIGNATURE not set")?;
    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/run/user/1000".to_string());
    Ok(PathBuf::from(runtime).join("hypr").join(&instance).join(".socket2.sock"))
}

/// Send a command to Hyprland and return the response
#[cfg(unix)]
pub async fn hyprctl(command: &str) -> Result<String> {
    let socket_path = hyprland_socket()?;
    let mut stream = UnixStream::connect(&socket_path)
        .await
        .with_context(|| format!("failed to connect to Hyprland socket at {:?}", socket_path))?;

    stream.write_all(command.as_bytes()).await?;
    stream.shutdown().await?;

    let mut reader = BufReader::new(stream);
    let mut response = String::new();
    reader.read_line(&mut response).await?;
    Ok(response.trim().to_string())
}

/// Send a JSON request to Hyprland (for `j/` prefixed commands)
#[cfg(unix)]
pub async fn hyprctl_json<T: for<'de> Deserialize<'de>>(command: &str) -> Result<T> {
    let raw = hyprctl(&format!("j/{}", command)).await?;
    serde_json::from_str(&raw).with_context(|| format!("failed to parse Hyprland JSON response: {}", &raw[..raw.len().min(200)]))
}

/// Spawn the event listener that reads from Hyprland's event socket
#[cfg(unix)]
pub async fn spawn_event_listener(
    mut callback: impl FnMut(HyprlandEvent) + Send + 'static,
) -> Result<tokio::task::JoinHandle<()>> {
    let socket_path = hyprland_event_socket()?;
    let handle = tokio::spawn(async move {
        loop {
            match connect_and_read_events(&socket_path, &mut callback).await {
                Ok(()) => info!("event socket disconnected, reconnecting..."),
                Err(e) => {
                    error!("event listener error: {e:#}, reconnecting in 1s...");
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
            }
        }
    });
    Ok(handle)
}

#[cfg(unix)]
async fn connect_and_read_events(
    socket_path: &PathBuf,
    callback: &mut (impl FnMut(HyprlandEvent) + Send),
) -> Result<()> {
    let stream = UnixStream::connect(socket_path)
        .await
        .with_context(|| format!("failed to connect to event socket at {:?}", socket_path))?;

    let mut reader = BufReader::new(stream);
    let mut line = String::new();

    loop {
        line.clear();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            break Ok(());
        }
        let trimmed = line.trim();
        if let Some(event) = parse_event(trimmed) {
            debug!("hyprland event: {:?}", event);
            callback(event);
        }
    }
}

// Non-unix stubs: Hyprland only runs on Linux, so the socket layer
// is compiled out elsewhere. These keep `cargo check` green on Windows.
#[cfg(not(unix))]
pub async fn hyprctl(_command: &str) -> Result<String> {
    Err(anyhow::anyhow!(
        "hyprctl needs a unix socket (Hyprland only runs on Linux)"
    ))
}

#[cfg(not(unix))]
pub async fn hyprctl_json<T: for<'de> Deserialize<'de>>(
    _command: &str,
) -> Result<T> {
    Err(anyhow::anyhow!(
        "hyprctl needs a unix socket (Hyprland only runs on Linux)"
    ))
}

#[cfg(not(unix))]
pub async fn spawn_event_listener(
    _callback: impl FnMut(HyprlandEvent) + Send + 'static,
) -> Result<tokio::task::JoinHandle<()>> {
    Err(anyhow::anyhow!(
        "event listener needs a unix socket (Hyprland only runs on Linux)"
    ))
}

#[derive(Debug, Clone)]
pub enum HyprlandEvent {
    WorkspaceChanged(u32),
    WorkspaceCreated(u32),
    WorkspaceDestroyed(u32),
    WindowOpened(String),
    WindowClosed(String),
    WindowFocused(String),
    ActiveMonitorChanged(String),
    ConfigReloaded,
    Unknown(String),
}

fn parse_event(line: &str) -> Option<HyprlandEvent> {
    let (event, data) = line.split_once(">>")?;
    Some(match event {
        "workspace" => HyprlandEvent::WorkspaceChanged(data.trim().parse().ok()?),
        "workspacev2" => {
            let id = data.split(',').next()?.trim().parse().ok()?;
            HyprlandEvent::WorkspaceChanged(id)
        }
        "createworkspace" | "createworkspacev2" => {
            let id = data.split(',').next()?.trim().parse().ok()?;
            HyprlandEvent::WorkspaceCreated(id)
        }
        "destroyworkspace" | "destroyworkspacev2" => {
            let id = data.split(',').next()?.trim().parse().ok()?;
            HyprlandEvent::WorkspaceDestroyed(id)
        }
        "openwindow" => HyprlandEvent::WindowOpened(data.split(',').next().unwrap_or("").to_string()),
        "closewindow" => HyprlandEvent::WindowClosed(data.to_string()),
        "activewindow" | "activewindowv2" => HyprlandEvent::WindowFocused(data.to_string()),
        "activespecial" => HyprlandEvent::ActiveMonitorChanged(data.to_string()),
        "configreloaded" => HyprlandEvent::ConfigReloaded,
        _ => HyprlandEvent::Unknown(line.to_string()),
    })
}
