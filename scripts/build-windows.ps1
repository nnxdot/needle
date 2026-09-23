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
    $version = (Select-String -LiteralPath (Join-Path $workspace 'Cargo.toml') -Pattern '^version = "(.+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
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
    $zip = Join-Path $workspace "dist\Needle-$version-windows-x64.zip"
    Compress-Archive -LiteralPath $package -DestinationPath $zip -Force
    $artifacts = @($zip)

    # The installer, when Inno Setup is installed (winget install JRSoftware.InnoSetup).
    $iscc = @(
        (Join-Path ${env:ProgramFiles(x86)} 'Inno Setup 6\ISCC.exe'),
        (Join-Path $env:ProgramFiles 'Inno Setup 6\ISCC.exe'),
        (Join-Path $env:LOCALAPPDATA 'Programs\Inno Setup 6\ISCC.exe')
    ) | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
    $setup = Join-Path $workspace "dist\Needle-Setup-$version.exe"
    if ($iscc) {
        & $iscc "/DAppVersion=$version" "/DSource=$package" (Join-Path $workspace 'packaging\needle.iss')
        if ($LASTEXITCODE) { throw 'Installer build failed.' }
        $artifacts += $setup
    } else {
        Write-Warning 'Inno Setup 6 is not installed, so no installer was made. Install it with: winget install JRSoftware.InnoSetup'
    }

    # SHA256SUMS.txt: Needle's updater installs only a download that matches this list.
    $sums = Join-Path $workspace 'dist\SHA256SUMS.txt'
    Get-FileHash -Algorithm SHA256 -LiteralPath $artifacts | ForEach-Object { '{0}  {1}' -f $_.Hash.ToLower(), (Split-Path $_.Path -Leaf) } | Set-Content -LiteralPath $sums

    # winget manifests, ready to submit to microsoft/winget-pkgs once the release is published.
    if ($iscc) {
        $hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $setup).Hash
        $manifests = Join-Path $workspace "dist\winget\$version"
        New-Item -ItemType Directory -Path $manifests -Force | Out-Null
        $id = 'nnxdot.Needle'
        $url = "https://github.com/nnxdot/needle/releases/download/v$version/Needle-Setup-$version.exe"
        @"
PackageIdentifier: $id
PackageVersion: $version
DefaultLocale: en-US
ManifestType: version
ManifestVersion: 1.6.0
"@ | Set-Content -LiteralPath (Join-Path $manifests "$id.yaml")
        @"
PackageIdentifier: $id
PackageVersion: $version
InstallerType: inno
Scope: user
InstallModes:
  - interactive
  - silent
UpgradeBehavior: install
FileExtensions: [flac, mp3, m4a, aac, ogg, opus, wav, aiff, aif, wv, ape, dsf, cue]
Installers:
  - Architecture: x64
    InstallerUrl: $url
    InstallerSha256: $hash
ManifestType: installer
ManifestVersion: 1.6.0
"@ | Set-Content -LiteralPath (Join-Path $manifests "$id.installer.yaml")
        @"
PackageIdentifier: $id
PackageVersion: $version
PackageLocale: en-US
Publisher: nnxdot
PackageName: Needle
PackageUrl: https://github.com/nnxdot/needle
License: See THIRD-PARTY-NOTICES.md
ShortDescription: A local music player for Windows.
Tags: [music, player, flac, lyrics, chromecast, dlna, airplay]
ManifestType: defaultLocale
ManifestVersion: 1.6.0
"@ | Set-Content -LiteralPath (Join-Path $manifests "$id.locale.en-US.yaml")
    }
    Write-Output "Built Needle $version in $(Join-Path $workspace 'dist')"
} finally { Pop-Location }
