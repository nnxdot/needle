param([switch]$SkipChecks)
$ErrorActionPreference = 'Stop'
$workspace = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$cargoCommand = Get-Command cargo -ErrorAction SilentlyContinue
$cargoExecutable = if ($cargoCommand) { $cargoCommand.Source } else { Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe' }
if (-not (Test-Path -LiteralPath $cargoExecutable)) { throw 'Install Rust and the Visual Studio C++ build tools first.' }
Push-Location -LiteralPath $workspace
try {
    if (-not $SkipChecks) {
        & $cargoExecutable fmt --all --check
        if ($LASTEXITCODE) { throw 'Formatting check failed.' }
        & $cargoExecutable test -p needle-core --locked
        if ($LASTEXITCODE) { throw 'Core tests failed.' }
        & $cargoExecutable clippy --workspace --all-targets --locked -- -D warnings
        if ($LASTEXITCODE) { throw 'Clippy failed.' }
    }
    & $cargoExecutable build --release -p needle --locked
    if ($LASTEXITCODE) { throw 'Release build failed.' }
    $package = Join-Path $workspace 'dist\Needle'
    New-Item -ItemType Directory -Path $package -Force | Out-Null
    Copy-Item -LiteralPath (Join-Path $workspace 'target\release\needle-desktop.exe') -Destination (Join-Path $package 'Needle.exe')
    Copy-Item -LiteralPath (Join-Path $workspace 'target\release\needle.exe') -Destination (Join-Path $package 'needle-cli.exe')
    # Runtime libraries the build placed next to the executables (ONNX Runtime's DirectML).
    Get-ChildItem -Path (Join-Path $workspace 'target\release') -Filter '*.dll' | Copy-Item -Destination $package
    foreach ($document in @('README.md','PROPOSAL.md','IMPLEMENTATION.md','VALIDATION.md','THIRD-PARTY-NOTICES.md')) {
        Copy-Item -LiteralPath (Join-Path $workspace $document) -Destination $package
    }
    Copy-Item -LiteralPath (Join-Path $workspace 'layouts') -Destination $package -Recurse -Force
    Copy-Item -LiteralPath (Join-Path $workspace 'third-party') -Destination $package -Recurse -Force
    Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $package 'Needle.exe'),(Join-Path $package 'needle-cli.exe') | ForEach-Object { '{0}  {1}' -f $_.Hash, (Split-Path $_.Path -Leaf) } | Set-Content -LiteralPath (Join-Path $package 'SHA256SUMS.txt')
    Compress-Archive -LiteralPath $package -DestinationPath (Join-Path $workspace 'dist\Needle-0.8.0-windows-x64.zip') -Force
    Write-Output "Built $package"
} finally { Pop-Location }
