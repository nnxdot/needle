param(
    [Parameter(Mandatory = $true)]
    [string]$ApkPath,
    [Parameter(Mandatory = $true)]
    [string]$BuildToolsDirectory,
    [string]$OutputPath,
    [string]$KeyDirectory = (Join-Path $env:USERPROFILE '.android/needle-release')
)

$ErrorActionPreference = 'Stop'
if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) {
    throw 'This signing helper stores its password with Windows user encryption.'
}
$source = (Resolve-Path -LiteralPath $ApkPath).Path
$buildTools = (Resolve-Path -LiteralPath $BuildToolsDirectory).Path
if (-not $OutputPath) {
    if (-not $source.EndsWith('-unsigned.apk')) { throw 'Specify OutputPath for this APK filename.' }
    $OutputPath = $source.Substring(0, $source.Length - '-unsigned.apk'.Length) + '.apk'
}
$destination = [IO.Path]::GetFullPath($OutputPath)
if ($source -eq $destination) { throw 'The signed output must be separate from the unsigned source.' }
$badging = & (Join-Path $buildTools 'aapt.exe') dump badging $source
if ($LASTEXITCODE -ne 0) { throw 'The source APK cannot be inspected.' }
if ($badging -match '^application-debuggable') { throw 'The source must be a release APK.' }

$keyFolder = [IO.Path]::GetFullPath($KeyDirectory)
New-Item -ItemType Directory -Path $keyFolder -Force | Out-Null
$identity = [Security.Principal.WindowsIdentity]::GetCurrent().User
$grant = '*' + $identity.Value + ':(OI)(CI)F'
& icacls.exe $keyFolder /inheritance:r /grant:r $grant | Out-Null
if ($LASTEXITCODE -ne 0) { throw 'Could not restrict access to the signing key folder.' }

$keystore = Join-Path $keyFolder 'needle-release.p12'
$passwordFile = Join-Path $keyFolder 'password.clixml'
$keyExists = Test-Path -LiteralPath $keystore
$passwordExists = Test-Path -LiteralPath $passwordFile
if ($keyExists -ne $passwordExists) {
    throw 'The signing key or its encrypted password is missing. Restore the existing pair; do not replace the key.'
}
$createKey = -not $keyExists
if ($createKey) {
    $randomBytes = New-Object byte[] 48
    $random = [Security.Cryptography.RandomNumberGenerator]::Create()
    try { $random.GetBytes($randomBytes) } finally { $random.Dispose() }
    $password = ConvertTo-SecureString ([Convert]::ToBase64String($randomBytes)) -AsPlainText -Force
    [Array]::Clear($randomBytes, 0, $randomBytes.Length)
} else {
    $password = Import-Clixml -LiteralPath $passwordFile
}

$priorPassword = $env:NEEDLE_ANDROID_SIGNING_PASSWORD
try {
    $env:NEEDLE_ANDROID_SIGNING_PASSWORD = [Net.NetworkCredential]::new('', $password).Password
    if ($createKey) {
        & keytool -genkeypair -keystore $keystore -storetype PKCS12 -alias needle-release `
            -keyalg RSA -keysize 4096 -sigalg SHA256withRSA -validity 10000 `
            -dname 'CN=Needle, O=nnx' -storepass:env NEEDLE_ANDROID_SIGNING_PASSWORD
        if ($LASTEXITCODE -ne 0) { throw 'Could not create the release signing key.' }
        try { $password | Export-Clixml -LiteralPath $passwordFile }
        catch {
            Remove-Item -LiteralPath $keystore
            throw
        }
    }
    & (Join-Path $buildTools 'apksigner.bat') sign --ks $keystore --ks-key-alias needle-release `
        --ks-pass env:NEEDLE_ANDROID_SIGNING_PASSWORD --key-pass env:NEEDLE_ANDROID_SIGNING_PASSWORD `
        --v4-signing-enabled false --out $destination $source
    if ($LASTEXITCODE -ne 0) { throw 'Release APK signing failed.' }
} finally {
    if ($null -eq $priorPassword) { Remove-Item Env:NEEDLE_ANDROID_SIGNING_PASSWORD -ErrorAction SilentlyContinue }
    else { $env:NEEDLE_ANDROID_SIGNING_PASSWORD = $priorPassword }
    $password.Dispose()
}
& (Join-Path $buildTools 'apksigner.bat') verify --verbose --print-certs $destination
if ($LASTEXITCODE -ne 0) { throw 'Signed APK verification failed.' }
& (Join-Path $buildTools 'zipalign.exe') -c -P 16 4 $destination
if ($LASTEXITCODE -ne 0) { throw 'Signed APK alignment verification failed.' }
Get-FileHash -LiteralPath $destination -Algorithm SHA256
