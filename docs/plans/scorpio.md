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
2. [ ] Rust workspace and interfaces (core types, platform traits).
3. [ ] Core in Rust with tests (crosshair, raster, center, focus, launchers).
4. [ ] Windows platform layer.
5. [ ] Linux platform layer.
6. [ ] App state machine, store, tray, hotkeys, Tauri commands.
7. [ ] Settings UI: tabs Crosshair, Games, Actions, Keybinds, System, About;
       switches, dropdowns, sliders, colour picker, keybind fields; one
       shape language.
8. [ ] Style and UX audit (`docs/style-audit.md`), applied.
9. [ ] Windows: build on the gaming PC, run, drive, measure.
10. [ ] Linux: build on the VPS, run under Xvfb + openbox, drive, measure.
11. [ ] Independent review, fixes, report.

## Dropped from CrossOver's feature set

Image presets, custom images, tint, reticle overlay, SVG customisation, theme
choice, slab colours, app size, duplicate crosshairs, follow mouse, tilt,
sounds, update checks, announcements, macOS, portable mode.
