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

Linux (VPS, Xvfb): the platform thread alone used 0 CPU ticks over 3 s and
about 3.8 MB resident. The full app has not been measured there yet.
