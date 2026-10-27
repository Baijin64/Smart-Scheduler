param(
    [ValidateSet("x64", "arm64")][string]$Arch = "x64",
    [switch]$SkipCargoBuild,
    [string]$CustomExe
)

$ErrorActionPreference = "Stop"
$ProjectRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $ProjectRoot

$env:Path += ";$env:USERPROFILE\.cargo\bin"

# Extract package version from Cargo.toml
$CargoToml = Get-Content (Join-Path $ProjectRoot "Cargo.toml") -Raw
if ($CargoToml -match 'version\s*=\s*"([^"]+)"') {
    $PkgVersion = $matches[1] + ".0"
} else {
    $PkgVersion = "0.2.2.0"
}

# Resolve binary path
if ($CustomExe) {
    $ExePath = $CustomExe
} elseif ($Arch -eq "arm64") {
    $ExePath = Join-Path $ProjectRoot "target\aarch64-pc-windows-msvc\release\smart-scheduler.exe"
    if (-not (Test-Path $ExePath)) {
        $GnuLlvmExe = Join-Path $ProjectRoot "target\aarch64-pc-windows-gnullvm\release\smart-scheduler.exe"
        if (Test-Path $GnuLlvmExe) { $ExePath = $GnuLlvmExe }
    }
} else {
    $ExePath = Join-Path $ProjectRoot "target\release\smart-scheduler.exe"
}

# Build binary if not skipped
if (-not $SkipCargoBuild -and -not $CustomExe) {
    Write-Host "[1/4] Building Release smart-scheduler.exe ($Arch) ..." -ForegroundColor Cyan
    if ($Arch -eq "arm64") {
        cargo build --release --target aarch64-pc-windows-msvc
    } else {
        cargo build --release
    }
} else {
    Write-Host "[1/4] Using pre-built binary: $ExePath" -ForegroundColor Cyan
}

if (-not (Test-Path $ExePath)) {
    throw "Executable not found at: $ExePath. If testing/building in another process, ensure build has finished or provide -CustomExe <path>."
}

$MsixStage = Join-Path $ProjectRoot "msix"
$AssetsDir = Join-Path $MsixStage "Assets"
New-Item -ItemType Directory -Force -Path $AssetsDir | Out-Null
Remove-Item (Join-Path $MsixStage "*.toml") -Force -ErrorAction SilentlyContinue
Remove-Item (Join-Path $MsixStage "*.exe") -Force -ErrorAction SilentlyContinue
Copy-Item -Force $ExePath (Join-Path $MsixStage "smart-scheduler.exe")

# Update AppxManifest.xml ProcessorArchitecture and Version
$ManifestPath = Join-Path $MsixStage "AppxManifest.xml"
$ManifestContent = Get-Content $ManifestPath -Raw -Encoding utf8
$ManifestContent = [System.Text.RegularExpressions.Regex]::Replace($ManifestContent, 'ProcessorArchitecture="[^"]+"', "ProcessorArchitecture=`"$Arch`"")
$ManifestContent = [System.Text.RegularExpressions.Regex]::Replace($ManifestContent, 'Version="[^"]+"', "Version=`"$PkgVersion`"")
Set-Content -Path $ManifestPath -Value $ManifestContent -Encoding utf8

Write-Host "[2/4] Generating MSIX PNG icon assets ..." -ForegroundColor Cyan
Add-Type -AssemblyName System.Drawing

function New-LogoPng([string]$OutPath, [int]$Size) {
    $bmp = New-Object System.Drawing.Bitmap($Size, $Size)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $g.TextRenderingHint = [System.Drawing.Text.TextRenderingHint]::AntiAliasGridFit
    $g.Clear([System.Drawing.Color]::Transparent)

    $pad = [int]($Size * 0.08)
    $diam = $Size - ($pad * 2)
    $brushBg = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(255, 16, 185, 129))
    $g.FillEllipse($brushBg, $pad, $pad, $diam, $diam)

    # Draw crisp white "S" in center
    $brushFg = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::White)
    $fontSize = [float]($Size * 0.50)
    $font = New-Object System.Drawing.Font("Segoe UI", $fontSize, [System.Drawing.FontStyle]::Bold, [System.Drawing.GraphicsUnit]::Pixel)
    $sf = New-Object System.Drawing.StringFormat
    $sf.Alignment = [System.Drawing.StringAlignment]::Center
    $sf.LineAlignment = [System.Drawing.StringAlignment]::Center
    $rect = New-Object System.Drawing.RectangleF(0, 0, $Size, $Size)
    $g.DrawString("S", $font, $brushFg, $rect, $sf)

    $bmp.Save($OutPath, [System.Drawing.Imaging.ImageFormat]::Png)
    $font.Dispose()
    $g.Dispose()
    $bmp.Dispose()
}

New-LogoPng (Join-Path $AssetsDir "StoreLogo.png") 50
New-LogoPng (Join-Path $AssetsDir "Square44x44Logo.png") 44
New-LogoPng (Join-Path $AssetsDir "Square150x150Logo.png") 150

Write-Host "[3/4] Packing MSIX via makeappx.exe ($Arch) ..." -ForegroundColor Cyan
$SdkBin = Get-ChildItem "C:\Program Files (x86)\Windows Kits\10\bin\*\x64\makeappx.exe" | Sort-Object FullName -Descending | Select-Object -First 1 -ExpandProperty DirectoryName
$MakeAppx = Join-Path $SdkBin "makeappx.exe"
$SignTool = Join-Path $SdkBin "signtool.exe"

$MsixOut = Join-Path $ProjectRoot "SmartScheduler_${PkgVersion}_${Arch}.msix"
& $MakeAppx pack /d $MsixStage /p $MsixOut /o | Out-Host
if ($LASTEXITCODE -ne 0) { throw "makeappx failed" }

Write-Host "[4/4] Creating/reusing self-signed cert and signing MSIX ..." -ForegroundColor Cyan
$Subject = "CN=DerekLi"
$Cert = Get-ChildItem Cert:\CurrentUser\My | Where-Object { $_.Subject -eq $Subject -and $_.NotAfter -gt (Get-Date) } | Select-Object -First 1
if (-not $Cert) {
    $Cert = New-SelfSignedCertificate `
        -Type Custom `
        -Subject $Subject `
        -KeyUsage DigitalSignature `
        -FriendlyName "Smart Scheduler Dev Cert" `
        -CertStoreLocation "Cert:\CurrentUser\My" `
        -TextExtension @("2.5.29.37={text}1.3.6.1.5.5.7.3.3", "2.5.29.19={text}")
}

$CerPath = Join-Path $ProjectRoot "SmartScheduler_Cert.cer"
Export-Certificate -Cert $Cert -FilePath $CerPath -Force | Out-Null

& $SignTool sign /fd SHA256 /sha1 $Cert.Thumbprint $MsixOut | Out-Host
if ($LASTEXITCODE -ne 0) { throw "signtool failed" }

$ExeSize = (Get-Item $ExePath).Length
$MsixSize = (Get-Item $MsixOut).Length
Write-Host ""
Write-Host "MSIX Build Complete ($Arch)!" -ForegroundColor Green
Write-Host "  Executable:   $ExePath ($ExeSize bytes)"
Write-Host "  MSIX Package: $MsixOut ($MsixSize bytes)"
Write-Host "  Certificate:  $CerPath"
