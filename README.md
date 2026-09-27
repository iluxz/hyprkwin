# hyprkwin

hyprland and kde had a baby. a rust daemon that exposes the `org.kde.KWin`
D-Bus interface and translates it to hyprland IPC — so you can run
`plasmashell` on hyprland.

## status

v0.1.0, early. virtual desktops + basic window ops work. plasmashell
integration untested. `cargo check` is green (msvc + gnu-adjacent paths;
real target is linux).

## build

```sh
cargo build --release
```

## run (on hyprland)

```sh
HYPRLAND_INSTANCE_SIGNATURE=... ./target/release/hyprkwin
```

then launch `plasmashell`. the daemon claims `org.kde.KWin` on the session
bus and forwards desktop switches, kills, and reloads to `hyprctl`.

## what it covers

- `/KWin` — current/next/previous desktop, kill window, reconfigure, keep-above, compositing info
- `/VirtualDesktopManager` — count, names, create/remove
- `/Compositor` — always-on (hyprland never stops compositing)
- event socket listener keeps desktop state in sync

## not covered (yet)

- per-window keep-above/below (pin acts on the active window)
- desktop names/renames, real multi-monitor mapping
- anything wayland-protocol-shaped (this is D-Bus only)
