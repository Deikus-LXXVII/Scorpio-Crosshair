# Checks a running Scorpio against docs/budget.md. Run on Windows while
# Scorpio is running with its settings window closed:
#   powershell -ExecutionPolicy Bypass -File tools\measure-windows.ps1 [-AfterSettings]
# -AfterSettings uses the budget for after the settings window was opened
# and closed once (the WebView2 runtime keeps a few threads loaded).
param([switch]$AfterSettings)
$budgetPrivateMB = if ($AfterSettings) { 12.8 } else { 8.6 }   # measured 10.2 / 6.9, +25 %
$budgetCpuMs = 50                                               # measured 0 ms over 10 s idle
$p = Get-Process scorpio -ErrorAction Stop
$c1 = $p.TotalProcessorTime.TotalMilliseconds; Start-Sleep 10; $p.Refresh()
$cpu = $p.TotalProcessorTime.TotalMilliseconds - $c1
$mb = $p.PrivateMemorySize64 / 1MB
$web = @(Get-CimInstance Win32_Process -Filter "Name='msedgewebview2.exe'" | Where-Object { $_.CommandLine -like '*com.lxxvii.scorpio*' }).Count
"private {0:N1} MB (budget {1}), cpu {2} ms over 10 s (budget {3}), web view processes {4} (budget 0)" -f $mb, $budgetPrivateMB, $cpu, $budgetCpuMs, $web
if ($mb -gt $budgetPrivateMB -or $cpu -gt $budgetCpuMs -or $web -gt 0) { Write-Error 'over budget'; exit 1 }
