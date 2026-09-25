param(
    # A copy of a Needle data folder (never the real one).
    [Parameter(Mandatory)][string]$DataDir,
    [string]$Exe = '',
    # How long to let Needle run; with -Bench, the longest to wait for the benchmark.
    [int]$Seconds = 30,
    # Files to open, so Needle plays them at once.
    [string[]]$Files = @(),
    [string]$Log = '',
    # Run the built-in benchmark (NEEDLE_BENCH): Needle walks its pages and closes itself.
    [switch]$Bench
)
# Runs Needle with frame timing on (NEEDLE_FRAME_LOG, a GPUI patch) and prints the summaries,
# plus the processor time and memory Needle used. See PERFORMANCE.md.
$root = Split-Path -Parent $MyInvocation.MyCommand.Path
if (-not $Exe) { $Exe = Join-Path $root '..\target\release\needle-desktop.exe' }
if (-not $Log) { $Log = Join-Path $env:TEMP 'needle-frames.log' }
Remove-Item -LiteralPath $Log -ErrorAction SilentlyContinue
$env:NEEDLE_FRAME_LOG = $Log
if ($Bench) { $env:NEEDLE_BENCH = '1' }
$arguments = @('--data-dir', "`"$DataDir`"") + ($Files | ForEach-Object { "`"$_`"" })
$p = Start-Process $Exe -ArgumentList $arguments -PassThru
$cpu = 0; $memory = 0
$deadline = (Get-Date).AddSeconds($Seconds)
while (-not $p.HasExited -and (Get-Date) -lt $deadline) {
    Start-Sleep -Milliseconds 500
    $p.Refresh()
    if (-not $p.HasExited) {
        $cpu = $p.TotalProcessorTime.TotalSeconds
        $memory = [math]::Max($memory, [int]($p.PeakWorkingSet64 / 1MB))
    }
    if (-not $Bench -and (Get-Date) -ge $deadline) { break }
}
if (-not $p.HasExited) {
    taskkill /PID $p.Id | Out-Null
    if (-not $p.WaitForExit(10000)) { Stop-Process -Id $p.Id -Force }
}
Remove-Item Env:NEEDLE_FRAME_LOG
Remove-Item Env:NEEDLE_BENCH -ErrorAction SilentlyContinue
"cpu {0:N1} s, peak memory {1} MB" -f $cpu, $memory
Get-Content -LiteralPath $Log -ErrorAction SilentlyContinue
