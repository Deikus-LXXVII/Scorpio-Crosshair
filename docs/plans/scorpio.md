# Scorpio: plan

A crosshair overlay for Windows and Linux, written new with what CrossOver became
in the `game-focus` branch as a reference for behaviour, keeping its useful parts
and dropping the rest. Started
2026-11-21 as an overnight run while the owner sleeps.

## What the owner asked for

- A new program, **Scorpio**, for **Linux and Windows** only, with CrossOver's
  functionality.
- **No crosshair image presets.** The crosshair is CS2's crosshair system only.
- A deep style audit; remove features that don't earn their place (theme
  choice "and so on").
- **No checkboxes** anywhere: switches, dropdowns and the like.
- **No square shapes next to rounded corners**: one shape language.
- **Written from scratch.** CrossOver is a reference for behaviour only; none
  of its code, assets or icons are copied.
- **Licence: open source, no commercial use.** Anyone may use it, change it
  and publish their own versions, as long as nobody sells it or uses it
  commercially.

## Decisions

| Decision | Why |
| --- | --- |
| Electron 45 (current) instead of CrossOver's 14 | Security updates, modern Chromium (CSS, `structuredClone`, desktopCapturer in main). CrossOver stayed on 14 for its own history. |
| Sharp corners everywhere, switches included | The owner's brand (lxxvll.com) is a carved stone tablet; one shape language, as asked. |
| Own JSON settings store with a schema, not electron-preferences | Its window is gone; the schema also drives the settings UI and validates every value in main. |
| Pipette by `desktopCapturer` + cursor position | Works on Windows and Linux X11 without a PowerShell helper. |
| Foreground window: PowerShell helper on Windows, `xprop -spy` on X11 | Same line protocol, so `focus/match.js` stays pure and shared. |
| Wayland: focus following and global hotkeys say they need X11 | Wayland gives clients neither the active window nor global keys. |
| Licence: PolyForm Noncommercial 1.0.0 | The standard licence for software whose source is open but whose commercial use is not allowed. Strictly it is "source-available": the OSI definition of open source forbids a no-commercial clause. |
| No CrossOver code | The owner asked for CrossOver as a reference only. Every module is written new; the share-code test vectors are akiver/csgo-sharecode's (MIT), credited in the fixture. |

## Behaviour taken from CrossOver (as a reference)

- Transparent click-through overlay, locked/unlocked; unlocked it sits on a
  stone slab with one Center button, drag to move.
- Exact centring on the display's centre pixel; 1 px moves; next display.
- CS2 crosshair: static styles, length, half-step thickness, gap, outline mode,
  outline thickness and colour, dot, T style, colour and alpha, made-for screen
  height; share codes in (four formats) and out (cs2-v1).
- My crosshairs (saved by name) and pro players' crosshairs.
- Games: show only while a game is focused; 2-of-4 verification profiles with
  their own crosshair; launcher detection (Steam, Epic, GOG, Ubisoft, Xbox,
  Riot, EA, Battle.net on Windows; Steam, Lutris, Heroic on Linux); fullscreen
  windows; "Not a game".
- Actions: hide on ADS (hold, toggle, show only while aiming), resize on ADS,
  hide on a key (hold, toggle, until next aim).
- Keybinds; start with the system; hide from screen capture; hardware
  acceleration.
- One Settings window with its own title bar; tray icon opens it.

## Dropped

Image presets (500+ PNG/SVG), custom images, tint, reticle overlay, SVG
customisation, theme choice, slab colours, app size, duplicate crosshairs,
follow mouse, tilt, sounds, update checks, announcements, macOS support,
portable mode.

## Steps (each ends in a check)

1. [x] Scaffold: package.json, Electron 45, folders, fonts, PolyForm Noncommercial licence.
2. [ ] Pure core with tests: cs2, look, center, focus match, launchers, schema.
3. [ ] Main: store, overlay, hotkeys, actions, focus (Win + X11), launchers,
       pipette, tray, settings IPC.
4. [ ] Renderer: overlay; settings window with components (switch, select,
       slider, colour, keybind), tabs Crosshair, Games, Actions, Keybinds,
       System, About.
5. [ ] Style audit written down (docs/style-audit.md) and applied.
6. [ ] Windows: build NSIS on the gaming PC, install beside CrossOver, drive
       over CDP, screenshots.
7. [ ] Linux: build on the VPS, run under Xvfb + openbox, drive over CDP.
8. [ ] Independent review (Grok change-review), fixes, report.

## Progress log

Kept in `.audit/scorpio.tsv` (local), one row per decision with its evidence.
