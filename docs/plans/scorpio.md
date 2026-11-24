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
   Changed by the owner on 2026-11-21: the player's own image (PNG, JPG,
   WebP, GIF, SVG) may replace the CS2 crosshair; still no presets.
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
| Its client area (centre on the game window) | `GetClientRect`, `ClientToScreen` | `GetGeometry`, `TranslateCoordinates`, `QueryTree` (to find the frame) | Window manager data about the window's rectangle; no process is opened. |
| The game window moved or resized (only while "Centre on the game window" is on and a game is in front) | `SetWinEventHook(EVENT_OBJECT_LOCATIONCHANGE, WINEVENT_OUTOFCONTEXT)` scoped with `idProcess` and `idThread` to the window in front, re-scoped on each foreground change, removed when not needed; the callback keeps only `OBJID_WINDOW` of that window | `StructureNotify` (ConfigureNotify) selected on the window in front and its frame, removed when not needed | Out of context, like the foreground hook: the system posts the event to Scorpio's own thread and nothing is loaded into the game. It reports only window-manager data (where a window is). Scoped to the one window's thread, other programs' moves never wake Scorpio, and a place already reported is not sent again, so a drag sends one event per real change. |
| Its executable | `NtQuerySystemInformation(SystemProcessIdInformation)` | `/proc/<pid>/exe`, else `cmdline` for Wine | No process handle is ever opened (`OpenProcess` is banned). |
| Exclusive fullscreen (Windows; for the settings window's status) | `SHQueryUserNotificationState` == `QUNS_RUNNING_D3D_FULL_SCREEN`, on a one-shot timer of Scorpio's own hidden window 2, 10 and 30 s after a fullscreen window comes to the front, stopping at the first yes | Not asked (X11 has no exclusive fullscreen that hides other windows) | A shell query apps use to hold back notifications; it reads no game data, opens no process and loads nothing. Three wake-ups at most per window change, none while the same window stays in front. |
| Aim button or key, hide key | Raw Input, `RIDEV_INPUTSINK`, on Scorpio's own hidden window | XInput2 raw events on the root | No low-level hooks (`WH_*_LL` are banned): the same input every game and Discord read. One device class at a time: the mouse for a mouse button, the keyboard only when the aim is a key, and nothing while hiding is off or no crosshair is up. |
| Hotkeys | `RegisterHotKey` | `XGrabKey` | Standard. |
| Overlay | Layered, topmost, click-through window (`WS_EX_LAYERED|TRANSPARENT|TOOLWINDOW|NOACTIVATE`) | Override-redirect ARGB window, empty input shape | A separate window over the game, like Discord's or Steam's. Never drawn into the game. |
| Colour pipette | One screen grab when the player asks | Same | Only on request. |
| Your image as the crosshair | Nothing new: the settings web view draws the file on a canvas and Scorpio decodes the PNG with the `png` crate into the same overlay window | Same | Files in Scorpio's own config folder; no game involved. |
| Report a problem: system version, browser for the GitHub issue, console for `--report` | `RtlGetVersion`, `ShellExecuteW("open", <issue address made by Scorpio>)`, `GetStdHandle` and `AttachConsole(ATTACH_PARENT_PROCESS)` | `/etc/os-release`, `/proc/sys/kernel/osrelease`, `xdg-open <issue address>` | About Scorpio's own system and process; only when the player asks; the address is built in Rust, never taken from the web view. Nothing is sent: the player attaches the file. |
| Report a problem: hardware, displays and presentation settings (runs only in child `scorpio --system-facts` when the player makes a report) | `RegGetValueW` (`ProcessorNameString`, `HwSchMode`, `DirectXUserGlobalSettings`, `AutoGameModeEnabled`), `GlobalMemoryStatusEx`, `__COMPAT_LAYER`, `EnumDisplaySettingsW`; `LoadLibraryW("dxgi.dll")` + `GetProcAddress("CreateDXGIFactory1")` + `IDXGIFactory1::EnumAdapters1`, `IDXGIAdapter1::GetDesc1` / `CheckInterfaceSupport(IDXGIDevice)` / `EnumOutputs`, `IDXGIOutput::GetDesc`, `IDXGIOutput2::SupportsOverlays`, `IDXGIOutput6::GetDesc1`, `FreeLibrary` | `/proc/cpuinfo`, `/proc/meminfo`, `/sys/class/drm/card*/device`, X11 RandR (`RRGetScreenResourcesCurrent`, `RRGetOutputInfo`, `RRGetCrtcInfo`, `RRGetOutputPrimary`), `_NET_WM_CM_S*` / `_NET_SUPPORTING_WM_CHECK` window properties, `XDG_SESSION_TYPE` | Reads only the player's own hardware, display and presentation settings when requested, in a short-lived child process. `dxgi.dll` is never in `scorpio.exe`'s import table and is never loaded in the main process. No D3D device is created and no other process is opened. |
| Report a problem: Scorpio's own resource cost | `GetCurrentProcess`, `GetProcessTimes`, `K32GetProcessMemoryInfo` (`PROCESS_MEMORY_COUNTERS_EX`), plus relaxed atomic counters on Scorpio's platform thread | `/proc/self/stat`, `/proc/self/auxv`, `/proc/self/smaps_rollup` (fallback `/proc/self/status`), plus the same atomic counters | Queries only Scorpio's own process (`GetCurrentProcess` pseudo-handle / `/proc/self`), only when the player makes a report; never opens another process and adds no timers or polling. |
| Log file | Writes to `%LOCALAPPDATA%\com.lxxvii.scorpio\logs` | `~/.local/share/com.lxxvii.scorpio/logs` | Scorpio's own files, capped at 2 MB. |
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
12. [x] Always show; your own image as the crosshair; a log file and "Report a problem" (with `scorpio --report`); centring on the game window, following it by events.

## Dropped from CrossOver's feature set

Image presets, tint, reticle overlay, SVG customisation, theme
choice, slab colours, app size, duplicate crosshairs, follow mouse, tilt,
sounds, update checks, announcements, macOS, portable mode.

## Known gaps (from the 2026-11-21 review)

- A game that turns borderless fullscreen after it came to the front is
  seen only at the next focus or display change, unless "Centre on the
  game window" is on and the game is already shown (then its location
  events say so).
- Whether the system delivers cursor location events to a hook scoped to
  a game's process and thread is not proven: an 8 s run on Windows saw
  none, but the mouse may not have moved. The callback drops them either
  way; each would still wake the platform thread once.
- A window that is still invisible when it comes to the front is ignored
  until the next focus change.
- One added UWP game (ApplicationFrameHost) matches every UWP app.
- On Linux each ConfigureNotify of a followed window costs about five X
  round trips before the "same place" check; a drag at 60 Hz works but
  could compare the event's own geometry first.
- Image files replaced during a session (a new image, each size tried)
  stay in `images/` until the next start prunes them.
- The image flow and the problem report have run only in headless
  Chromium with a stand-in for Scorpio, not in WebView2 or WebKitGTK.
- With hide-while-aiming on and a mouse button as the aim, raw mouse input
  wakes the platform thread for every movement packet. Not yet measured; add
  a figure to `docs/budget.md`. A key as the aim wakes it only on key presses.
- The aim key is bound by its position (`KeyboardEvent.code`), and read by
  scan code on Windows and evdev code on Linux (`platform::KEYS`), so it holds
  on every layout. Two limits: on Windows, AltGr arrives as a left Control
  plus a right Alt, so pressing AltGr in the field binds Left Ctrl (and Left
  Ctrl then aims too); WebView2 may never pass PrintScreen to the page, and
  Alt+PrintScreen sends SysRq, which is not in the table. Keys a program
  sends with no scan code (macro tools) are not read on Windows. Escape and
  Pause cannot be the aim: Escape cancels binding, Pause sends no release.

## Code signing (owner's decision, 2026-11-21)

No paid code signing certificate: the cost is not worth it for this
project. The Windows installer stays unsigned, so SmartScreen warns on
first run (the README and release notes say how to continue). Once the
owner has tested Scorpio and the fixes from that test are released, the
installer of that version is submitted to Microsoft for analysis, once,
not for every small release.
