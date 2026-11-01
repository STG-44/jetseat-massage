param([ValidateSet('x64','arm64')][string]$Architecture = 'x64')
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$installer = Get-ChildItem (Join-Path $projectRoot "dist/*-Windows-$Architecture-Setup.exe") | Select-Object -First 1
if (-not $installer) { throw 'Installer was not built' }
$installDir = Join-Path $env:TEMP "jetseat-install-smoke-$Architecture"
$process = Start-Process -FilePath $installer.FullName -ArgumentList @('/S', "/D=$installDir") -PassThru
if (-not $process.WaitForExit(60000)) { $process.Kill(); throw 'Installer timed out' }
if ($process.ExitCode -ne 0) { throw "Installer failed: $($process.ExitCode)" }
$binary = Join-Path $installDir 'jetseat-massage.exe'
if (-not (Test-Path $binary)) { throw 'Installed executable is missing' }
$reportPath = Join-Path $env:TEMP "jetseat-diagnostic-$Architecture.json"
$process = Start-Process -FilePath $binary -ArgumentList @('--diagnose', $reportPath) -PassThru
if (-not $process.WaitForExit(30000)) { $process.Kill(); throw 'Installed executable timed out' }
if ($process.ExitCode -ne 0) { throw "Installed executable failed: $($process.ExitCode)" }
$report = Get-Content $reportPath -Raw | ConvertFrom-Json
$expectedArch = if ($Architecture -eq 'arm64') { 'aarch64' } else { 'x86_64' }
if ($report.os -ne 'windows' -or $report.arch -ne $expectedArch) { throw 'Wrong installed OS/CPU architecture' }
Write-Output "Installed Windows binary ran successfully: $($report.version), $($report.arch)"
$uninstaller = Join-Path $installDir 'Uninstall.exe'
$process = Start-Process -FilePath $uninstaller -ArgumentList '/S' -PassThru
if (-not $process.WaitForExit(60000)) { $process.Kill(); throw 'Uninstaller timed out' }
for ($attempt = 0; $attempt -lt 50 -and (Test-Path $binary); $attempt++) { Start-Sleep -Milliseconds 200 }
if (Test-Path $binary) { throw 'Uninstaller left the executable behind' }
Write-Output 'Installer, native CPU executable, HID diagnostics and uninstaller: OK'
