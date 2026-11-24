# Problem reports: more facts, and our own place to send them

Owner's decisions, 2026-11-24:

- Reports go to our own small service on the VPS, not to a third party.
- Sending happens only when the player presses Send after reading the whole
  report. The old path stays: save the file and attach it to a GitHub issue.
- The report gains: hardware and displays, the Windows settings that change
  how games are presented, what Scorpio itself has cost since it started,
  and a field for what happened (with an optional contact).

Why now: a player reported losing about 1 % of frames with Scorpio. The
current report could not answer "which GPU, which display mode, was hide
while aiming on, how often did Scorpio wake up".

## What the player sees

The System tab's "Show the report" dialog gets two fields above the text:
"What happened" (required to send, up to 4000 characters) and "How to reach
you" (optional: Discord name or e-mail, up to 200 characters). The text
below updates as they type, so what they read is exactly what is sent.
Buttons: Copy, Save, Report on GitHub (as now), and Send. Send shows the
report's number (for example `SCR-7K2Q`) and keeps it in the dialog to copy.

## What the report adds

All of it is about the player's own computer and Scorpio's own process.
Nothing opens, reads or touches a game.

| Section | Windows | Linux |
| --- | --- | --- |
| Processor, memory | Registry `CentralProcessor\0\ProcessorNameString`, `GlobalMemoryStatusEx` | `/proc/cpuinfo` model name, `/proc/meminfo` |
| Graphics cards | DXGI `EnumAdapters1`: name, dedicated memory, driver version (`CheckInterfaceSupport`) | `/sys/class/drm/card*/device` vendor and device ids, driver name |
| Displays | DXGI outputs: resolution, refresh (`EnumDisplaySettingsW`), HDR (`IDXGIOutput6::GetDesc1`), overlay planes (`IDXGIOutput2::SupportsOverlays`) | RandR outputs the app already reads |
| Presentation settings | HAGS (`GraphicsDrivers\HwSchMode`), windowed-game optimisations (`DirectX\UserGpuPreferences`), Game Mode (`GameBar\AutoGameModeEnabled`), `__COMPAT_LAYER` | Session type, compositor name |
| Scorpio's cost | Own CPU time and private memory since start (`GetProcessTimes`, `GetProcessMemoryInfo`), wake-ups by cause (raw input, foreground, window moves, timers), time with a game in front, the highest raw-input rate seen in one second | Same from `/proc/self` and the same counters |

DXGI is never linked into `scorpio.exe`: an import would load `dxgi.dll` at
every start and break the idle memory budget. The hardware and display part
runs in a child process, `scorpio --system-facts`, which prints JSON and
exits. The main process starts it only when the player opens the report.
No D3D device is created; only the factory and its enumeration.

The wake-up counters are relaxed atomic adds on the platform thread, one per
message; nothing new wakes up to keep them.

## The service

`server/reports/` in this repository: one small program behind Caddy at
`https://reports.lxxvll.com/v1/reports`.

- `POST` JSON `{version, system, description, contact, text}`, at most
  512 KB. Answers `201 {"id": "SCR-…"}`.
- Stores each report as a row in SQLite with the time it arrived. The IP
  address is not stored; it is kept in memory only for the rate limit
  (5 reports an hour per address, 200 a day in all).
- Kept 180 days, then deleted.
- The owner reads reports with `tools/reports.sh` over ssh (list, show,
  close). A web page can come later.
- No secret in the app: anything shipped in it can be pulled out, so the
  limits above are the protection.

Scorpio sends with the `reqwest` client and bundled root certificates it
already uses for updates. The address is a constant in Rust; the web view
passes only the two fields, and Rust sends the report text it made and
showed, not text from the web view.

## Public text that changes

README ("Scorpio never sends anything by itself and has no telemetry"), the
site, `llms.txt`, and the anti-cheat review table in `scorpio.md` gain: a
report is sent only when you press Send, after you have read it; nothing is
sent otherwise; what the service keeps and for how long. A privacy note on
the site says the same.

## Steps

1. Report contents: the child process, the counters, the new sections, the
   description and contact fields. Useful alone through the GitHub path.
   Check: unit tests for the text and redaction; `cargo xwin check` and the
   Linux build; the report made on the gaming PC holds every section;
   `tools/measure-windows.ps1` shows idle memory and CPU unchanged.
2. The service and its deploy. Check: its tests (accepted, too large, rate
   limited, bad JSON); a real POST to the deployed address. Deploy needs the
   owner's go-ahead.
3. Send in the app, and the public text. Check: send from the gaming PC,
   read it back with `tools/reports.sh`.
4. Release 0.1.6.

## Progress (2026-11-24)

- Step 1 done (Neon, 6cc4e8e; Linux build fixed in f6008c1). Checked here:
  tests on macOS and Ubuntu, `cargo xwin build` with no `dxgi.dll` import,
  `--system-facts` on the VPS. Not yet run on the gaming PC.
- Step 2 done: deployed to the VPS (`usa-vps`), hardened after a Neon
  review (9ddc411). Limits as built: 10 attempts an hour per address or
  IPv6 /64, 200 stored reports a day, 4 requests at once, 1 GB ceiling.
  Every request gets an answer, so Caddy logs no upstream error with the
  caller's address; if the service itself is down, Caddy's error log
  (journald) would hold the address of a caller that tried then.
- Step 3 done in code (d7f27b4) and public text; the site and `llms.txt`
  go live with the release, since 0.1.5 has no Send.
- Three Neon review rounds (ee5ef23, 9ddc411 + d7f27b4, 31e979a + 2bd9d4d);
  every confirmed finding fixed. Left as is, inferred only: more than ~60
  callers at once could reach the unit's TasksMax=64 and get a dropped
  connection (Caddy would then log that caller's address); with Caddy
  buffering each body, the service's connections last milliseconds.
- Released as 0.1.6 on 2026-11-24, with the site (counter included).
  Checked on the gaming PC before release: the 0.1.6 `--system-facts`
  in the player's session read the CPU, three adapters, both displays
  (5120x1440 at 240 Hz with HDR, 3840x2160 at 60 Hz) and the presentation
  settings; over ssh it sees no displays, as a service session has none. A
  POST from the gaming PC got 201 and read back with `tools/reports.sh`,
  then was deleted. Not checked: Send pressed in the app's own window on
  Windows (the Rust send path is covered by the local HTTP tests).
- Left: `tools/measure-windows.ps1` on the installed 0.1.6.

## Open

- Idle memory and CPU of the installed 0.1.6 on Windows.
