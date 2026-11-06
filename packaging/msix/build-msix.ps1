<#
.SYNOPSIS
  Builds the Microsoft Store edition of Dear Us as an .msix package.

.DESCRIPTION
  1. Builds the app with the Store edition features (no self-updater, Dear Us Cloud, Store purchases).
  2. Lays out the package: the app, LGPL ffmpeg (downloaded at BUILD time, checksum-verified),
     its licence notice, tile/logo assets and AppxManifest.xml.
  3. Packs it with makeappx (Windows SDK) and, if makepri is available, adds scaled assets.

  The Store re-signs packages on submission, so signing is only needed to install a local
  test build (-CertPfx / -CertPassword with a certificate whose subject equals -Publisher).

.EXAMPLE
  ./packaging/msix/build-msix.ps1 `
    -IdentityName "12345DearUs.DearUs" `
    -Publisher "CN=00000000-0000-0000-0000-000000000000" `
    -PublisherDisplayName "Dear Us" `
    -CloudUrl "https://api.dearus.app"
#>
param(
  # default: packaging/msix/identity.json
  [string]$IdentityName,
  [string]$Publisher,
  [string]$PublisherDisplayName,
  [string]$CloudUrl = "https://api.dearus.app",
  [string]$OutDir = "dist-msix",
  [switch]$SkipBuild,
  [switch]$AllowUnpinnedFfmpeg,
  # A build to install on your own PC before the Store has it: test identity (unless
  # identity.json is filled in), signed with a freshly made self-signed certificate.
  [switch]$TestBuild,
  [string]$CertPfx,
  [string]$CertPassword
)

$ErrorActionPreference = "Stop"
$root = Resolve-Path (Join-Path $PSScriptRoot "../..")
$here = $PSScriptRoot
Set-Location $root

$identity = Get-Content (Join-Path $here "identity.json") -Raw | ConvertFrom-Json
if (-not $IdentityName) { $IdentityName = $identity.identityName }
if (-not $Publisher) { $Publisher = $identity.publisher }
if (-not $PublisherDisplayName) { $PublisherDisplayName = $identity.publisherDisplayName }
if ($TestBuild -and ($IdentityName -match 'XXXX' -or $Publisher -match 'XXXX')) {
  $IdentityName = "DearUs.TestBuild"
  $Publisher = "CN=DearUsTestBuild"
  Write-Warning "Test build: using a test identity. Fill in packaging/msix/identity.json for Store uploads."
}
if ($TestBuild) { $AllowUnpinnedFfmpeg = $true }
if (-not $CloudUrl) { $CloudUrl = "https://api.dearus.app" }
if ($IdentityName -match 'XXXX' -or $Publisher -match 'XXXX') {
  throw "Fill in packaging/msix/identity.json (or pass -IdentityName/-Publisher) with the values from Partner Center."
}
if ($Publisher -notmatch '^CN=') { throw "-Publisher must be the full publisher string from Partner Center, e.g. CN=XXXXXXXX-XXXX-..." }

# ---------------------------------------------------------------- version
$conf = Get-Content "src-tauri/tauri.conf.json" -Raw | ConvertFrom-Json
$parts = $conf.version.Split('.')
while ($parts.Count -lt 3) { $parts += "0" }
$version = "$($parts[0]).$($parts[1]).$($parts[2]).0"   # the Store requires the 4th part to be 0
Write-Host "Dear Us $version (Microsoft Store edition)"

# ---------------------------------------------------------------- build
if (-not $SkipBuild) {
  $env:DEARUS_CLOUD_URL = $CloudUrl
  npm ci
  if ($LASTEXITCODE -ne 0) { throw "npm ci failed" }
  npx tauri build --no-bundle --config src-tauri/tauri.microsoftstore.conf.json -- --no-default-features --features store
  if ($LASTEXITCODE -ne 0) { throw "tauri build failed" }
}
$exe = Get-ChildItem "src-tauri/target/release" -Filter "*.exe" -File |
  Where-Object { $_.Name -in @("dear-us.exe", "Dear Us.exe") } | Select-Object -First 1
if (-not $exe) { throw "Built app not found in src-tauri/target/release" }

# ---------------------------------------------------------------- layout
$layout = Join-Path $OutDir "layout"
if (Test-Path $layout) { Remove-Item $layout -Recurse -Force }
New-Item -ItemType Directory -Force -Path "$layout/Assets", "$layout/licenses/ffmpeg" | Out-Null
$exeName = "DearUs.exe"   # no spaces in the package's entry point
Copy-Item $exe.FullName (Join-Path $layout $exeName)

# ---------------------------------------------------------------- ffmpeg (LGPL)
$ff = Get-Content (Join-Path $here "ffmpeg-lgpl.json") -Raw | ConvertFrom-Json
$ffUrl = $ff.url
$ffSha = $ff.sha256
if (-not $ffUrl -or -not $ffSha) {
  if (-not $AllowUnpinnedFfmpeg) {
    throw "Pin the LGPL ffmpeg build in packaging/msix/ffmpeg-lgpl.json (url + sha256), or run once with -AllowUnpinnedFfmpeg to see the current build's SHA-256."
  }
  $ffUrl = $ff.fallbackUrl
  Write-Warning "Using the UNPINNED ffmpeg build $ffUrl. Pin it before submitting to the Store."
}
$zip = Join-Path $OutDir "ffmpeg-lgpl.zip"
Invoke-WebRequest -Uri $ffUrl -OutFile $zip -UseBasicParsing
$actual = (Get-FileHash $zip -Algorithm SHA256).Hash.ToLower()
if ($ffSha -and $actual -ne $ffSha.ToLower()) { throw "ffmpeg checksum mismatch: expected $ffSha, got $actual" }
if (-not $ffSha) { Write-Host "ffmpeg SHA-256 (put this in ffmpeg-lgpl.json together with a dated URL): $actual" }
$ffDir = Join-Path $OutDir "ffmpeg"
if (Test-Path $ffDir) { Remove-Item $ffDir -Recurse -Force }
Expand-Archive $zip -DestinationPath $ffDir
$bin = Get-ChildItem $ffDir -Recurse -Directory -Filter "bin" | Select-Object -First 1
if (-not $bin) { throw "No bin folder in the ffmpeg archive" }
# Refuse GPL builds: the Store edition must ship LGPL only.
$cfg = & (Join-Path $bin.FullName "ffmpeg.exe") -hide_banner -buildconf 2>&1 | Out-String
if ($cfg -match "--enable-gpl") { throw "This ffmpeg build is GPL (--enable-gpl). Use a win64-lgpl(-shared) build." }
Copy-Item (Join-Path $bin.FullName "ffmpeg.exe"), (Join-Path $bin.FullName "ffprobe.exe") $layout
Get-ChildItem $bin.FullName -Filter "*.dll" | Copy-Item -Destination $layout
$lic = Get-ChildItem $ffDir -Recurse -File | Where-Object { $_.Name -match '^(LICENSE|COPYING)' } | Select-Object -First 1
if ($lic) { Copy-Item $lic.FullName "$layout/licenses/ffmpeg/LICENSE.txt" }
@"
Dear Us includes FFmpeg (https://ffmpeg.org), licensed under the GNU Lesser General Public
License v2.1 or later ($($ff.license)). FFmpeg is used unmodified, as separate programs and
dynamically linked libraries in this folder, which you may replace with your own build.
Binary build: $ffUrl
Source code: $($ff.source)
"@ | Set-Content "$layout/licenses/ffmpeg/NOTICE.txt" -Encoding UTF8

# ---------------------------------------------------------------- assets
$icons = "src-tauri/icons"
$assets = @("StoreLogo", "Square44x44Logo", "Square71x71Logo", "Square150x150Logo", "Square310x310Logo", "Wide310x150Logo")
$makepri = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin" -Recurse -Filter makepri.exe -ErrorAction SilentlyContinue |
  Where-Object { $_.FullName -match '\\x64\\' } | Sort-Object FullName -Descending | Select-Object -First 1
foreach ($a in $assets) {
  if ($makepri -and (Test-Path "$icons/$a.scale-200.png")) {
    Copy-Item "$icons/$a.png" "$layout/Assets/$a.scale-100.png"
    Copy-Item "$icons/$a.scale-200.png" "$layout/Assets/$a.scale-200.png"
  } else {
    Copy-Item "$icons/$a.png" "$layout/Assets/$a.png"
  }
}

# ---------------------------------------------------------------- manifest
$manifest = Get-Content (Join-Path $here "AppxManifest.xml") -Raw
$values = @{
  "{{IDENTITY_NAME}}"          = $IdentityName
  "{{PUBLISHER}}"              = $Publisher
  "{{PUBLISHER_DISPLAY_NAME}}" = [System.Security.SecurityElement]::Escape($PublisherDisplayName)
  "{{VERSION}}"                = $version
  "{{EXECUTABLE}}"             = $exeName
}
foreach ($k in $values.Keys) { $manifest = $manifest.Replace($k, $values[$k]) }
if ($manifest -match '\{\{') { throw "AppxManifest.xml still has unfilled {{…}} values" }
Set-Content (Join-Path $layout "AppxManifest.xml") $manifest -Encoding UTF8

if ($makepri) {
  $priConfig = Join-Path $OutDir "priconfig.xml"
  & $makepri.FullName createconfig /cf $priConfig /dq en-US /o | Out-Null
  & $makepri.FullName new /pr $layout /cf $priConfig /mn (Join-Path $layout "AppxManifest.xml") /of (Join-Path $layout "resources.pri") /o
  if ($LASTEXITCODE -ne 0) { throw "makepri failed" }
} else {
  Write-Warning "makepri.exe not found: packaging without scaled (200%) assets."
}

# ---------------------------------------------------------------- pack
$makeappx = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin" -Recurse -Filter makeappx.exe -ErrorAction SilentlyContinue |
  Where-Object { $_.FullName -match '\\x64\\' } | Sort-Object FullName -Descending | Select-Object -First 1
if (-not $makeappx) { throw "makeappx.exe not found: install the Windows 10/11 SDK." }
$msix = Join-Path $OutDir "DearUs_$version`_x64.msix"
& $makeappx.FullName pack /d $layout /p $msix /o
if ($LASTEXITCODE -ne 0) { throw "makeappx failed" }

if ($TestBuild -and -not $CertPfx) {
  # self-signed certificate whose subject matches the package publisher
  $cert = New-SelfSignedCertificate -Type Custom -Subject $Publisher -KeyUsage DigitalSignature -FriendlyName "Dear Us test build" `
    -CertStoreLocation "Cert:\CurrentUser\My" -TextExtension @("2.5.29.37={text}1.3.6.1.5.5.7.3.3", "2.5.29.19={text}")
  $CertPassword = [guid]::NewGuid().ToString()
  $CertPfx = Join-Path $OutDir "DearUs-test.pfx"
  Export-PfxCertificate -Cert $cert -FilePath $CertPfx -Password (ConvertTo-SecureString $CertPassword -AsPlainText -Force) | Out-Null
  Export-Certificate -Cert $cert -FilePath (Join-Path $OutDir "DearUs-test-certificate.cer") | Out-Null
}
if ($CertPfx) {
  $signtool = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin" -Recurse -Filter signtool.exe -ErrorAction SilentlyContinue |
    Where-Object { $_.FullName -match '\\x64\\' } | Sort-Object FullName -Descending | Select-Object -First 1
  & $signtool.FullName sign /fd SHA256 /a /f $CertPfx /p $CertPassword $msix
  if ($LASTEXITCODE -ne 0) { throw "signing failed" }
}

if ($CertPfx -and $TestBuild) { Remove-Item $CertPfx -Force }  # never ship the private key
Write-Host ""
Write-Host "Done: $msix"
if ($TestBuild) {
  Write-Host "Test build: install DearUs-test-certificate.cer into 'Local Machine > Trusted People' once, then open the .msix."
} else {
  Write-Host "Upload it in Partner Center → your app → Packages (the Store signs it)."
}
