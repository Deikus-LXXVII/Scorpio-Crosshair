# Budget

What Scorpio costs while you play, measured on the owner's Windows 11
gaming PC (2026-11-21, release build, 2560x1440, 20 logical processors).
`tools/measure-windows.ps1` fails when a figure grows by more than 25 %; run
it at least a minute after start, after the first update check. Lower a
budget on purpose, in its own commit, after the figure improved.

| Figure | Measured | Budget |
| --- | --- | --- |
| Private memory, idle in the tray, settings never opened, after the first update check | 7.4 MB | 8.6 MB |
| Private memory, after the settings window was opened and closed | 9.2 MB | 11.5 MB |
| Threads, idle / after the settings window | 9 / 13 | 12 / 16 |
| CPU time over 30 s idle | 0 ms | 16 ms (one scheduler tick) |
| Web view processes with the settings window closed | 0 | 0 |
| `scorpio.exe` size | 6.1 MB (0.1.3, with 11 dictionaries and Latin Extended fonts) | 4.6 MB (over: 0.1.1 already measured 5.2 MB) |

Medians of three runs with the owner's bench script (25 s after start, then
30 s), and three steady-state runs 180 to 210 s after start (7.3 MB, 0 ms).
Working set is 30 MB, mostly shared system DLLs. The idle budget stays at
its older 8.6 MB, lower than 7.4 MB + 25 %.

Where the idle 7.4 MB goes:

- 4.4 MB is `AcLayers.dll`, the Windows compatibility-shim engine. This PC
  sets `__COMPAT_LAYER=DisableDXMaximizedWindowedMode` for the whole system
  (an AtlasOS tweak), so every program started on it loads the shim, and its
  writable pages count as private memory. Started without the variable,
  Scorpio uses 2.6 MB before its first update check.
- 2.6 MB is Scorpio's own: 0.9 MB of heap and other private allocations,
  0.3 MB of thread stacks (8 threads), 0.6 MB of writable DLL pages (40
  DLLs), 0.3 MB of writable shared sections, and about 0.5 MB that a walk of
  the address space does not show (page tables). Turning off the launcher
  scan, the tray, the shortcuts, single-instance or autostart one at a time
  each changed it by less than 0.1 MB.
- 0.4 MB stays after an update check: Winsock and DNS (0.13 MB) and what
  rustls and hyper keep. Ten checks 15 s apart stayed within 0.05 MB.

0.1.6, measured on 2026-11-25 after it had updated itself from 0.1.5 (so
started by the updater, without the system-wide `__COMPAT_LAYER`, and after
its first update check): 2.9 MB private, 7 threads, 0 ms CPU over 30 s, no
web view process. That matches the 2.6 MB + 0.4 MB above: the problem report
work added nothing to the idle cost.

What used to cost more, and the fix:

- Tauri's default async runtime starts a worker per logical processor (20
  here) at the first update check or settings window, and they never exit:
  1.5 MB and 20 threads. Scorpio now gives Tauri a runtime with one worker.
- Checking the server certificate with Windows' root store goes through the
  certificate chain engine, which keeps 1.45 MB and four threads and costs
  20 to 30 ms of CPU per check (about 3 ms with the bundled roots). Update checks now trust the root
  certificates bundled with Scorpio (`webpki-root-certs`) and fall back to the
  system's when those fail (an antivirus or proxy that re-signs HTTPS).
- The segment heap (set in `src-tauri/windows-manifest.xml`) saves 0.1 to
  0.2 MB against the old Windows heap.

With the settings window open, WebView2 adds six processes and about
210 MB; all of them exit when the window closes. WebView2's in-process part
(`EmbeddedBrowserWebView.dll`, three threads, about 1 MB of heap) stays
loaded after it closes, which is the 1.8 MB between the first two rows.

Linux (VPS, Ubuntu 24.04 under Xvfb + openbox, the .deb build, settings
never opened, 2026-11-21): GTK and WebKitGTK are loaded at start, so Linux
costs more than Windows.

| Figure | Measured | Budget |
| --- | --- | --- |
| Private dirty memory (anonymous) | 20.1 MB | 25 MB |
| PSS (shared libraries counted fairly) | 70.7 MB | 88 MB |
| CPU over 10 s idle | 2 ticks (20 ms) | 5 ticks |
| .deb size | 3.2 MB | 4 MB |
| AppImage size (bundles WebKitGTK) | 78 MB | 98 MB |

Measured with `/proc/<pid>/smaps_rollup` and `/proc/<pid>/stat`; there is
no Linux check script yet. Not measured again after the one-worker runtime
and the bundled root certificates, which apply to Linux too.
