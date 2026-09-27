mod hyprland;
mod kwin_dbus;

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

use kwin_dbus::{CompositorDBus, KWinDBus, KWinState, VirtualDesktopManager};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    info!("=== hyprkwin v0.1.0 ===");
    info!("Hyprland -> KWin D-Bus bridge for KDE Plasma shell");

    // Check we're on Hyprland
    if std::env::var("HYPRLAND_INSTANCE_SIGNATURE").is_err() {
        warn!("HYPRLAND_INSTANCE_SIGNATURE not set — are you running Hyprland?");
        warn!("hyprkwin will start but won't be able to talk to Hyprland");
    }

    // Sync initial state from Hyprland (unix only; elsewhere we start blank)
    let state = Arc::new(RwLock::new(KWinState::default()));
    #[cfg(unix)]
    if let Ok(workspace) = hyprland::hyprctl_json::<HyprlandWorkspace>("activeworkspace").await {
        let mut s = state.write().await;
        s.current_desktop = workspace.id;
        s.desktop_count = workspace.id.max(1);
    }

    // Connect to D-Bus session bus
    let connection = zbus::Connection::session()
        .await
        .expect("failed to connect to D-Bus session bus");

    // Register KWin interface on /KWin
    let kwin = KWinDBus::new(state.clone());
    connection
        .object_server()
        .at("/KWin", kwin)
        .await
        .expect("failed to register /KWin");

    // Register VirtualDesktopManager on /VirtualDesktopManager
    let vdm = VirtualDesktopManager::new(state.clone());
    connection
        .object_server()
        .at("/VirtualDesktopManager", vdm)
        .await
        .expect("failed to register /VirtualDesktopManager");

    // Register Compositor on /Compositor
    let compositor = CompositorDBus;
    connection
        .object_server()
        .at("/Compositor", compositor)
        .await
        .expect("failed to register /Compositor");

    // Request the well-known name
    let name = connection
        .request_name("org.kde.KWin")
        .await
        .expect("failed to request org.kde.KWin name");
    info!("acquired D-Bus name: org.kde.KWin ({:?})", name);

    info!("bridge ready — waiting for plasmashell...");

    // Spawn event listener to sync state from Hyprland events (unix only)
    #[cfg(unix)]
    {
        let state_clone = state.clone();
        hyprland::spawn_event_listener(move |event| {
            let state = state_clone.clone();
            tokio::spawn(async move {
                handle_event(&state, event).await;
            });
        })
        .await?;
    }
    #[cfg(not(unix))]
    warn!("event listener disabled: Hyprland sockets need a unix platform");

    // Run forever
    tokio::signal::ctrl_c().await?;
    info!("shutting down");
    Ok(())
}

async fn handle_event(state: &Arc<RwLock<KWinState>>, event: hyprland::HyprlandEvent) {
    use hyprland::HyprlandEvent::*;
    match event {
        WorkspaceChanged(id) => {
            state.write().await.current_desktop = id;
        }
        WorkspaceCreated(id) => {
            let mut s = state.write().await;
            if id > s.desktop_count {
                s.desktop_count = id;
            }
        }
        WindowFocused(addr) => {
            state.write().await.focused_window = addr;
        }
        _ => {}
    }
}

// Hyprland workspace JSON response
#[derive(serde::Deserialize)]
struct HyprlandWorkspace {
    id: u32,
    #[allow(dead_code)]
    name: String,
}
