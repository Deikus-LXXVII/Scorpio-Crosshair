# Scorpio: plan

A crosshair overlay for Windows and Linux, written new, with what CrossOver
became in its `game-focus` branch as a reference for behaviour only. Started
2026-11-21 as an overnight run while the owner sleeps.

## What the owner asked for, in priority order

1. **Anti-cheat safety first.** Scorpio never touches a game's or an
   anti-cheat's process: no handles to it, no injection, no reading its memory,
   no hooks in it, no synthetic input. Anything that could make an anti-cheat
   look at Scorpio is out.
2. **Light, fast, stable.** Minimal memory and CPU, nothing polling that
   doesn't need to.
3. **UX that is logical and obvious.**
4. Linux and Windows only. CS2's crosshair system only (no image presets).
   Remove features that don't earn their place (theme choice and so on).
5. No checkboxes: switches, dropdowns and the like. No square shapes next to
   rounded ones: one shape language.
6. Written from scratch. Licence: open source, no commercial use; anyone may
   use, change and publish their own versions.

## Architecture

One small native process does everything that runs while you play; the
settings interface is a web view that exists only while it is open.

```
scorpio (Rust, Tauri 2)
├── core/            pure logic, unit-tested, no OS calls
│   ├── crosshair    CS2 settings, share codes, shapes in pixels
│   ├── raster       shapes -> premultiplied RGBA bitmap
│   ├── center       exact centring
│   ├── focus        2-of-4 matching, focus state machine
│   └── launchers    install folders from launcher files
├── platform/        the only code that talks to the OS
│   ├── windows      layered overlay window, foreground event hook,
│   │                Raw Input, process image path without a handle
│   └── linux        X11 override-redirect overlay, active-window events,
│                    XInput2 raw events
├── app              state machine: locked, focus, ADS -> overlay
├── store            settings file, defaults, validation
└── ui/              settings web view (HTML, CSS, JS), opened on demand
```

### Anti-cheat review of every OS call

| Need | Windows | Linux (X11) | Why it is safe |
| --- | --- | --- | --- |
| Window in front | `SetWinEventHook(EVENT_SYSTEM_FOREGROUND, WINEVENT_OUTOFCONTEXT)` | `PropertyNotify` on the root's `_NET_ACTIVE_WINDOW` | Out of context: nothing is loaded into any other process. Event driven: no polling. |
| Its pid, title, class, fullscreen | `GetWindowThreadProcessId`, `GetWindowTextW`, `GetClassNameW`, `GetWindowRect`, `MonitorFromWindow` | `_NET_WM_PID`, `WM_CLASS`, `_NET_WM_NAME`, `_NET_WM_STATE` | Window manager data; no process is opened. |
| Its executable | `NtQuerySystemInformation(SystemProcessIdInformation)` | `/proc/<pid>/exe`, else `cmdline` for Wine | No process handle is ever opened (`OpenProcess` is banned). |
| Aim button, hide key | Raw Input, `RIDEV_INPUTSINK`, on Scorpio's own hidden window | XInput2 raw events on the root | No low-level hooks (`WH_*_LL` are banned): the same input every game and Discord read. |
| Hotkeys | `RegisterHotKey` | `XGrabKey` | Standard. |
| Overlay | Layered, topmost, click-through window (`WS_EX_LAYERED|TRANSPARENT|TOOLWINDOW|NOACTIVATE`) | Override-redirect ARGB window, empty input shape | A separate window over the game, like Discord's or Steam's. Never drawn into the game. |
| Colour pipette | One screen grab when the player asks | Same | Only on request. |
| Crosshair in OBS (opt-in) | A local HTTP server on 127.0.0.1:47117 serving a Browser source page and server-sent events | Same | Talks only to OBS on the same computer; no game involved. Off by default; no thread or socket while off. |
| Hide from screen capture (owner's exception, 2026-11-21) | `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)` on the overlay | Not available | **Not risk-free**: cheat overlays use the same call to hide from anti-cheat screenshots. Off by default; the switch says so. Kept at the owner's request. |

Banned outright: `OpenProcess` on other processes, `ReadProcessMemory`,
`WriteProcessMemory`, DLL injection, `SetWindowsHookEx` of any kind,
`SendInput`/`keybd_event`, kernel drivers, DirectX/Vulkan hooks, scanning
other processes' modules.

### Lightness budget

While playing: one process, target under 20 MB private memory, 0% CPU when
nothing changes (every input is an event, not a poll). The settings web view
is created when opened and destroyed when closed. Measured and recorded in
`docs/budget.md`; a check fails when it grows by more than 25%.

## Steps

1. [x] Licence, plan, JS reference of the core (to be ported).
2. [x] Rust workspace and interfaces (core types, platform traits).
3. [x] Core in Rust with tests (crosshair, raster, center, focus, launchers).
4. [x] Windows platform layer (built and probed on the gaming PC).
5. [x] Linux platform layer (built and probed on the VPS under Xvfb).
6. [x] App state machine, store, tray, hotkeys, Tauri commands.
7. [x] Settings UI: tabs Crosshair, Games, Controls, System (see the style audit);
       switches, dropdowns, sliders, colour picker, keybind fields; one
       shape language.
8. [x] Style and UX audit (`docs/style-audit.md`), applied.
9. [x] Windows: build on the gaming PC, run, drive, measure (idle and settings; not yet in a real game).
10. [x] Linux: .deb and AppImage built on the VPS, run under Xvfb + openbox, measured (settings window not driven yet).
11. [x] Independent review (Opus, Grok paused), fixes; open items under Known gaps.

## Dropped from CrossOver's feature set

Image presets, custom images, tint, reticle overlay, SVG customisation, theme
choice, slab colours, app size, duplicate crosshairs, follow mouse, tilt,
sounds, update checks, announcements, macOS, portable mode.

## Known gaps (from the 2026-11-21 review)

- A game that turns borderless fullscreen after it came to the front is
  seen only at the next focus or display change (Windows watches only
  foreground and display changes; location events would wake Scorpio on
  every cursor move).
- A window that is still invisible when it comes to the front is ignored
  until the next focus change.
- One added UWP game (ApplicationFrameHost) matches every UWP app.
- No log file: failures such as a settings write error are silent.
- With hide-while-aiming on, raw mouse input wakes the platform thread for
  every movement packet. Not yet measured; add a figure to `docs/budget.md`.
