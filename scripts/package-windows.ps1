param(
    [ValidateSet('x64', 'arm64')][string]$Architecture = 'x64'
)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
Push-Location $projectRoot
try {
    $metadata = (& cargo metadata --no-deps --format-version 1 --locked) | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0) { throw 'cargo metadata failed' }
    $version = ($metadata.packages | Where-Object name -eq 'jetseat-massage').version
    $target = if ($Architecture -eq 'arm64') { 'aarch64-pc-windows-msvc' } else { 'x86_64-pc-windows-msvc' }
    & cargo build --release --locked --target $target
    if ($LASTEXITCODE -ne 0) { throw 'Windows build failed' }
    $targetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $projectRoot 'target' }
    $binary = (Resolve-Path (Join-Path $targetDir "$target/release/jetseat-massage.exe")).Path
    $bytes = [System.IO.File]::ReadAllBytes($binary)
    $peOffset = [BitConverter]::ToInt32($bytes, 0x3c)
    $machine = [BitConverter]::ToUInt16($bytes, $peOffset + 4)
    $expectedMachine = if ($Architecture -eq 'arm64') { 0xaa64 } else { 0x8664 }
    if ($machine -ne $expectedMachine) { throw 'Wrong CPU architecture in Windows binary' }
    $info = [Diagnostics.FileVersionInfo]::GetVersionInfo($binary)
    if (-not $info.FileVersion.StartsWith($version)) { throw 'EXE version resource is missing' }
    $dist = Join-Path $projectRoot 'dist'
    New-Item -ItemType Directory -Force -Path $dist | Out-Null
    $output = Join-Path $dist "JetSeat-Massage-$version-Windows-$Architecture-Setup.exe"
    $candidates = @(
        (Join-Path ${env:ProgramFiles(x86)} 'NSIS/makensis.exe'),
        (Join-Path $env:ProgramFiles 'NSIS/makensis.exe')
    )
    $makensis = $candidates | Where-Object { Test-Path $_ } | Select-Object -First 1
    if (-not $makensis) {
        $command = Get-Command makensis -ErrorAction SilentlyContinue
        if ($command) { $makensis = $command.Source }
    }
    if (-not $makensis) { throw 'Install NSIS first: choco install nsis -y' }
    $icon = (Resolve-Path 'assets/icons/app.ico').Path
    $license = (Resolve-Path 'LICENSE').Path
    $readme = (Resolve-Path 'README.md').Path
    $fontLicense = (Resolve-Path 'assets/fonts/OFL.txt').Path
    & $makensis '/V3'  "/DARCH=$Architecture" "/DAPP_VERSION=$version" "/DBINARY=$binary" "/DOUTPUT=$output" "/DICON=$icon" "/DLICENSE_FILE=$license" "/DREADME_FILE=$readme" "/DFONT_LICENSE=$fontLicense" "$projectRoot/packaging/windows/installer.nsi"
    if ($LASTEXITCODE -ne 0) { throw 'Windows installer packaging failed' }
    $portable = Join-Path $dist "portable-Windows-$Architecture"
    New-Item -ItemType Directory -Force -Path $portable | Out-Null
    Copy-Item $binary (Join-Path $portable 'jetseat-massage.exe') -Force
    Copy-Item LICENSE,README.md $portable -Force
    Copy-Item assets/fonts/OFL.txt (Join-Path $portable 'OFL-NotoSansCJK.txt') -Force
    Compress-Archive -Path "$portable/*" -DestinationPath (Join-Path $dist "JetSeat-Massage-$version-Windows-$Architecture.zip") -Force
    Write-Output "Ready: $output"
} finally {
    Pop-Location
}
