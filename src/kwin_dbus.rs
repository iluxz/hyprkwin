use zbus::interface;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

use crate::hyprland;

/// Shared state between D-Bus methods
#[derive(Clone)]
pub struct KWinState {
    pub current_desktop: u32,
    pub desktop_count: u32,
    pub focused_window: String,
}

impl Default for KWinState {
    fn default() -> Self {
        Self {
            current_desktop: 1,
            desktop_count: 1,
            focused_window: String::new(),
        }
    }
}

/// The main KWin D-Bus interface that plasmashell talks to
pub struct KWinDBus {
    state: Arc<RwLock<KWinState>>,
}

impl KWinDBus {
    pub fn new(state: Arc<RwLock<KWinState>>) -> Self {
        Self { state }
    }
}

#[interface(name = "org.kde.KWin")]
impl KWinDBus {
    /// Return the current virtual desktop number
    async fn current_desktop(&self) -> u32 {
        self.state.read().await.current_desktop
    }

    /// Switch to a specific virtual desktop
    async fn set_current_desktop(&self, desktop: u32) {
        info!("setCurrentDesktop -> {}", desktop);
        let _ = hyprland::hyprctl(&format!("dispatch workspace {}", desktop)).await;
        self.state.write().await.current_desktop = desktop;
    }

    /// Switch to the next desktop
    async fn next_desktop(&self) {
        info!("nextDesktop");
        let _ = hyprland::hyprctl("dispatch workspace +1").await;
        let mut state = self.state.write().await;
        state.current_desktop += 1;
    }

    /// Switch to the previous desktop
    async fn previous_desktop(&self) {
        info!("previousDesktop");
        let _ = hyprland::hyprctl("dispatch workspace -1").await;
        let mut state = self.state.write().await;
        if state.current_desktop > 1 {
            state.current_desktop -= 1;
        }
    }

    /// Cascade all windows
    async fn cascade_desktop(&self) {
        info!("cascadeDesktop (no-op on Hyprland)");
    }

    /// Unclutter the desktop
    async fn unclutter_desktop(&self) {
        info!("unclutterDesktop (no-op on Hyprland)");
    }

    /// Kill the focused window
    async fn kill_window(&self) {
        info!("killWindow");
        let _ = hyprland::hyprctl("dispatch killactive").await;
    }

    /// Reload KWin config (reloads Hyprland config)
    async fn reconfigure(&self) {
        info!("reconfigure -> reload Hyprland");
        let _ = hyprland::hyprctl("reload").await;
    }

    /// Set whether a window should stay on top
    async fn set_keep_above(&self, window_id: u32, keep_above: bool) {
        info!("setKeepAbove window={} keep={}", window_id, keep_above);
        // pin/unpin act on the active window; callers should focus first
        let rule = if keep_above { "pin" } else { "unpin" };
        let _ = hyprland::hyprctl(&format!("dispatch {}", rule)).await;
    }

    /// Set whether a window should stay below
    async fn set_keep_below(&self, window_id: u32, keep_below: bool) {
        info!("setKeepBelow window={} keep={}", window_id, keep_below);
    }

    /// Support information
    async fn support_information(&self) -> String {
        "hyprkwin v0.1.0 — Hyprland pretending to be KWin".to_string()
    }
}

/// D-Bus interface for virtual desktop management
pub struct VirtualDesktopManager {
    state: Arc<RwLock<KWinState>>,
}

impl VirtualDesktopManager {
    pub fn new(state: Arc<RwLock<KWinState>>) -> Self {
        Self { state }
    }
}

#[interface(name = "org.kde.KWin.VirtualDesktopManager")]
impl VirtualDesktopManager {
    /// Number of desktops
    async fn count(&self) -> u32 {
        self.state.read().await.desktop_count
    }

    /// Names of desktops
    async fn names(&self) -> Vec<String> {
        let count = self.state.read().await.desktop_count;
        (1..=count).map(|i| format!("Desktop {}", i)).collect()
    }

    /// Create a new desktop
    async fn create_desktop(&self) {
        info!("createDesktop");
        let mut state = self.state.write().await;
        state.desktop_count += 1;
    }

    /// Remove a desktop by index
    async fn remove_desktop(&self, _id: u32) {
        info!("removeDesktop (limited support on Hyprland)");
    }

    /// Set desktop name
    async fn set_desktop_name(&self, _id: &str, _name: &str) {
        info!("setDesktopName (limited support on Hyprland)");
    }
}

/// D-Bus interface for compositing control
pub struct CompositorDBus;

#[interface(name = "org.kde.kwin.Compositing")]
impl CompositorDBus {
    async fn is_active(&self) -> bool {
        true // Hyprland is always compositing
    }

    async fn is_compositing_possible(&self) -> bool {
        true
    }

    async fn is_open_gl_broken(&self) -> bool {
        false
    }

    async fn suspend(&self) {
        info!("compositor suspend (no-op on Hyprland)");
    }

    async fn resume(&self) {
        info!("compositor resume (no-op on Hyprland)");
    }

    async fn reinitialize(&self) {
        info!("compositor reinitialize -> reload Hyprland");
        let _ = hyprland::hyprctl("reload").await;
    }
}
