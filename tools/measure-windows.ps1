# Checks a running Scorpio against docs/budget.md. Run on Windows at least a
# minute after Scorpio started (the first update check runs at 30 s), with
# its settings window closed and without using the PC for the 30 s it takes:
#   powershell -ExecutionPolicy Bypass -File tools\measure-windows.ps1 [-AfterSettings]
# -AfterSettings uses the budget for after the settings window was opened
# and closed once (WebView2's in-process part stays loaded).
param([switch]$AfterSettings)
$budgetPrivateMB = if ($AfterSettings) { 11.5 } else { 8.6 }   # measured 9.2 / 7.4, +25 % (idle kept at its older, lower budget)
$budgetThreads = if ($AfterSettings) { 16 } else { 12 }         # measured 13 / 9, +25 %
$budgetCpuMs = 16                                               # one 15.6 ms scheduler tick; measured 0 ms over 30 s idle
$p = Get-Process scorpio -ErrorAction Stop
$c1 = $p.TotalProcessorTime.TotalMilliseconds; Start-Sleep 30; $p.Refresh()
$cpu = $p.TotalProcessorTime.TotalMilliseconds - $c1
$mb = $p.PrivateMemorySize64 / 1MB
$threads = $p.Threads.Count
$web = @(Get-CimInstance Win32_Process -Filter "Name='msedgewebview2.exe'" | Where-Object { $_.CommandLine -like '*com.lxxvii.scorpio*' }).Count
"private {0:N1} MB (budget {1}), working set {2:N1} MB, threads {3} (budget {4}), cpu {5} ms over 30 s (budget {6}), web view processes {7} (budget 0)" -f $mb, $budgetPrivateMB, ($p.WorkingSet64 / 1MB), $threads, $budgetThreads, $cpu, $budgetCpuMs, $web
if ($mb -gt $budgetPrivateMB -or $threads -gt $budgetThreads -or $cpu -gt $budgetCpuMs -or $web -gt 0) { Write-Error 'over budget'; exit 1 }
