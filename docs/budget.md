# Budget

What Scorpio costs while you play, measured on the owner's Windows 11
gaming PC (2026-11-21, release build, 2560x1440). `tools/measure-windows.ps1`
fails when a figure grows by more than 25 %. Lower a budget on purpose, in
its own commit, after the figure improved.

| Figure | Measured | Budget |
| --- | --- | --- |
| Private memory, idle in the tray, settings never opened | 6.9 MB | 8.6 MB |
| Private memory, after the settings window was opened and closed | 10.2 MB | 12.8 MB |
| CPU time over 10 s idle | 0 ms | 50 ms |
| Web view processes with the settings window closed | 0 | 0 |
| `scorpio.exe` size | 3.7 MB | 4.6 MB |

With the settings window open, WebView2 adds six processes and about
240 MB; all of them exit when the window closes.

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
no Linux check script yet.
