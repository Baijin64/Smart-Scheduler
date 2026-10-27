param(
    [ValidateSet("x64", "arm64")][string]$Arch = "x64",
    [switch]$SkipCargoBuild,
    [string]$CustomExe,
    [switch]$StoreBuild,
    [string]$StorePackageName,
    [string]$StorePublisher,
    [string]$StorePublisherDisplayName,
    [switch]$CreateBundle
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
    $PkgVersion = "0.2.3.0"
}

# Resolve Windows SDK packaging tools
$SdkBin = Get-ChildItem "C:\Program Files (x86)\Windows Kits\10\bin\*\x64\makeappx.exe" -ErrorAction SilentlyContinue | Sort-Object FullName -Descending | Select-Object -First 1 -ExpandProperty DirectoryName
if (-not $SdkBin) {
    throw "Windows 10/11 SDK tools (makeappx.exe) not found. Please install the Windows SDK."
}
$MakeAppx = Join-Path $SdkBin "makeappx.exe"
$SignTool = Join-Path $SdkBin "signtool.exe"
$MakePri  = Join-Path $SdkBin "makepri.exe"

# If user requested to bundle existing x64 and arm64 MSIX packages
if ($CreateBundle) {
    Write-Host "[Bundle] Creating Multi-Architecture .msixbundle ($PkgVersion) ..." -ForegroundColor Cyan
    $BundleInDir = Join-Path $ProjectRoot "target\bundle_staging"
    $BundleOutPath = Join-Path $ProjectRoot "SmartScheduler_${PkgVersion}.msixbundle"

    if (Test-Path $BundleInDir) { Remove-Item -Recurse -Force $BundleInDir }
    New-Item -ItemType Directory -Force -Path $BundleInDir | Out-Null

    $X64Msix = Join-Path $ProjectRoot "SmartScheduler_${PkgVersion}_x64.msix"
    $Arm64Msix = Join-Path $ProjectRoot "SmartScheduler_${PkgVersion}_arm64.msix"

    $FoundAny = $false
    if (Test-Path $X64Msix) {
        Copy-Item -Force $X64Msix (Join-Path $BundleInDir (Split-Path $X64Msix -Leaf))
        $FoundAny = $true
        Write-Host "  -> Added x64 payload: $(Split-Path $X64Msix -Leaf)" -ForegroundColor Green
    }
    if (Test-Path $Arm64Msix) {
        Copy-Item -Force $Arm64Msix (Join-Path $BundleInDir (Split-Path $Arm64Msix -Leaf))
        $FoundAny = $true
        Write-Host "  -> Added ARM64 payload: $(Split-Path $Arm64Msix -Leaf)" -ForegroundColor Green
    }

    if (-not $FoundAny) {
        throw "Neither $X64Msix nor $Arm64Msix was found. Please build them first before bundling."
    }

    & $MakeAppx bundle /d $BundleInDir /p $BundleOutPath /bv $PkgVersion /o | Out-Host
    if ($LASTEXITCODE -ne 0) { throw "makeappx bundle failed" }
    Remove-Item -Recurse -Force $BundleInDir

    # If dev certificate is present, sign the bundle
    $Subject = if ($StorePublisher) { $StorePublisher } else { "CN=8BE064BB-20F6-4B24-8FB4-41C94B229A53" }
    $Cert = Get-ChildItem Cert:\CurrentUser\My | Where-Object { $_.Subject -eq $Subject -and $_.NotAfter -gt (Get-Date) } | Select-Object -First 1
    if ($Cert -and -not $StoreBuild) {
        & $SignTool sign /fd SHA256 /sha1 $Cert.Thumbprint $BundleOutPath | Out-Host
    }

    $BundleSize = (Get-Item $BundleOutPath).Length
    Write-Host ""
    Write-Host "MSIX Bundle Complete!" -ForegroundColor Green
    Write-Host "  Bundle Path: $BundleOutPath ($BundleSize bytes)"
    return
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
    $TargetX64Exe = Join-Path $ProjectRoot "target\x86_64-pc-windows-msvc\release\smart-scheduler.exe"
    $DefaultExe = Join-Path $ProjectRoot "target\release\smart-scheduler.exe"
    if ((Test-Path $TargetX64Exe) -and ((-not (Test-Path $DefaultExe)) -or ((Get-Item $TargetX64Exe).LastWriteTime -gt (Get-Item $DefaultExe).LastWriteTime))) {
        $ExePath = $TargetX64Exe
    } else {
        $ExePath = $DefaultExe
    }
}

# Build binary if not skipped
if (-not $SkipCargoBuild -and -not $CustomExe) {
    Write-Host "[1/5] Building Release smart-scheduler.exe ($Arch) ..." -ForegroundColor Cyan
    if ($Arch -eq "arm64") {
        cargo build --release --target aarch64-pc-windows-msvc
    } else {
        cargo build --release
    }
} else {
    Write-Host "[1/5] Using pre-built binary: $ExePath" -ForegroundColor Cyan
}

if (-not (Test-Path $ExePath)) {
    throw "Executable not found at: $ExePath. If testing/building in another process, ensure build has finished or provide -CustomExe <path>."
}

$MsixStage = Join-Path $ProjectRoot "msix"
$AssetsDir = Join-Path $MsixStage "Assets"
New-Item -ItemType Directory -Force -Path $AssetsDir | Out-Null
Remove-Item (Join-Path $MsixStage "*.toml") -Force -ErrorAction SilentlyContinue
Remove-Item (Join-Path $MsixStage "*.exe") -Force -ErrorAction SilentlyContinue
Remove-Item (Join-Path $MsixStage "resources.pri") -Force -ErrorAction SilentlyContinue
Copy-Item -Force $ExePath (Join-Path $MsixStage "smart-scheduler.exe")

# Update AppxManifest.xml accurately without corrupting MinVersion or MaxVersionTested
Write-Host "[2/5] Updating AppxManifest.xml metadata ($Arch, v$PkgVersion) ..." -ForegroundColor Cyan
$ManifestPath = Join-Path $MsixStage "AppxManifest.xml"
$ManifestContent = Get-Content $ManifestPath -Raw -Encoding utf8

$ManifestContent = [System.Text.RegularExpressions.Regex]::Replace($ManifestContent, '(?<=<Identity\b[^>]*?\bProcessorArchitecture=")[^"]+', $Arch)
$ManifestContent = [System.Text.RegularExpressions.Regex]::Replace($ManifestContent, '(?<=<Identity\b[^>]*?\bVersion=")[^"]+', $PkgVersion)

if ($StorePackageName) {
    $ManifestContent = [System.Text.RegularExpressions.Regex]::Replace($ManifestContent, '(?<=<Identity\b[^>]*?\bName=")[^"]+', $StorePackageName)
}
if ($StorePublisher) {
    $ManifestContent = [System.Text.RegularExpressions.Regex]::Replace($ManifestContent, '(?<=<Identity\b[^>]*?\bPublisher=")[^"]+', $StorePublisher)
}
if ($StorePublisherDisplayName) {
    $ManifestContent = [System.Text.RegularExpressions.Regex]::Replace($ManifestContent, '(?<=<PublisherDisplayName>)[^<]+', $StorePublisherDisplayName)
}

# Preserve standard Windows 10 build 19041 (2004) min version required for allowElevation
$ManifestContent = [System.Text.RegularExpressions.Regex]::Replace($ManifestContent, '(?<=<TargetDeviceFamily\b[^>]*?\bMinVersion=")[^"]+', "10.0.19041.0")

Set-Content -Path $ManifestPath -Value $ManifestContent -Encoding utf8

Write-Host "[3/5] Generating complete multi-scale & unplated MSIX icon assets ..." -ForegroundColor Cyan
Add-Type -AssemblyName System.Drawing

function Generate-IconAssets([string]$dir) {
    $assets = @(
        # Store logo
        @{ Name = "StoreLogo.png"; Width = 50; Height = 50; Mode = "badge" },
        @{ Name = "StoreLogo.scale-100.png"; Width = 50; Height = 50; Mode = "badge" },
        @{ Name = "StoreLogo.scale-200.png"; Width = 100; Height = 100; Mode = "badge" },
        # App list / Taskbar / Tray 44x44
        @{ Name = "Square44x44Logo.png"; Width = 44; Height = 44; Mode = "badge" },
        @{ Name = "Square44x44Logo.scale-100.png"; Width = 44; Height = 44; Mode = "badge" },
        @{ Name = "Square44x44Logo.scale-200.png"; Width = 88; Height = 88; Mode = "badge" },
        @{ Name = "Square44x44Logo.targetsize-24.png"; Width = 24; Height = 24; Mode = "badge" },
        @{ Name = "Square44x44Logo.targetsize-44.png"; Width = 44; Height = 44; Mode = "badge" },
        @{ Name = "Square44x44Logo.targetsize-48.png"; Width = 48; Height = 48; Mode = "badge" },
        @{ Name = "Square44x44Logo.targetsize-256.png"; Width = 256; Height = 256; Mode = "badge" },
        # Taskbar unplated (no background plate)
        @{ Name = "Square44x44Logo.altform-unplated_targetsize-24.png"; Width = 24; Height = 24; Mode = "badge" },
        @{ Name = "Square44x44Logo.altform-unplated_targetsize-44.png"; Width = 44; Height = 44; Mode = "badge" },
        @{ Name = "Square44x44Logo.altform-unplated_targetsize-48.png"; Width = 48; Height = 48; Mode = "badge" },
        @{ Name = "Square44x44Logo.altform-unplated_targetsize-256.png"; Width = 256; Height = 256; Mode = "badge" },
        # Medium tile 150x150
        @{ Name = "Square150x150Logo.png"; Width = 150; Height = 150; Mode = "badge" },
        @{ Name = "Square150x150Logo.scale-100.png"; Width = 150; Height = 150; Mode = "badge" },
        @{ Name = "Square150x150Logo.scale-200.png"; Width = 300; Height = 300; Mode = "badge" },
        # Wide tile 310x150
        @{ Name = "Wide310x150Logo.png"; Width = 310; Height = 150; Mode = "wide" },
        @{ Name = "Wide310x150Logo.scale-200.png"; Width = 620; Height = 300; Mode = "wide" }
    )

    foreach ($item in $assets) {
        $w = $item.Width
        $h = $item.Height
        $bmp = New-Object System.Drawing.Bitmap($w, $h)
        $g = [System.Drawing.Graphics]::FromImage($bmp)
        $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
        $g.TextRenderingHint = [System.Drawing.Text.TextRenderingHint]::AntiAliasGridFit
        $g.Clear([System.Drawing.Color]::Transparent)

        $minDim = [Math]::Min($w, $h)
        $pad = [int]($minDim * 0.08)
        $diam = $minDim - ($pad * 2)
        $cx = [int](($w - $diam) / 2)
        $cy = [int](($h - $diam) / 2)

        $brushBg = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(255, 16, 185, 129)) # Emerald-500
        $g.FillEllipse($brushBg, $cx, $cy, $diam, $diam)

        $brushFg = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::White)
        $fontSize = [float]($diam * 0.52)
        $font = New-Object System.Drawing.Font("Segoe UI", $fontSize, [System.Drawing.FontStyle]::Bold, [System.Drawing.GraphicsUnit]::Pixel)
        $sf = New-Object System.Drawing.StringFormat
        $sf.Alignment = [System.Drawing.StringAlignment]::Center
        $sf.LineAlignment = [System.Drawing.StringAlignment]::Center
        $rect = New-Object System.Drawing.RectangleF($cx, $cy, $diam, $diam)
        $g.DrawString("S", $font, $brushFg, $rect, $sf)

        $out = Join-Path $dir $item.Name
        $bmp.Save($out, [System.Drawing.Imaging.ImageFormat]::Png)
        $font.Dispose()
        $g.Dispose()
        $bmp.Dispose()
    }

    # Generate standalone 300x300 Store listing icon for Microsoft Partner Center web portal
    $StoreListingBmp = New-Object System.Drawing.Bitmap(300, 300)
    $sg = [System.Drawing.Graphics]::FromImage($StoreListingBmp)
    $sg.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $sg.TextRenderingHint = [System.Drawing.Text.TextRenderingHint]::AntiAliasGridFit
    $sg.Clear([System.Drawing.Color]::Transparent)
    $sBrushBg = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(255, 16, 185, 129))
    $sg.FillEllipse($sBrushBg, 24, 24, 252, 252)
    $sBrushFg = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::White)
    $sFont = New-Object System.Drawing.Font("Segoe UI", 130.0, [System.Drawing.FontStyle]::Bold, [System.Drawing.GraphicsUnit]::Pixel)
    $sSf = New-Object System.Drawing.StringFormat
    $sSf.Alignment = [System.Drawing.StringAlignment]::Center
    $sSf.LineAlignment = [System.Drawing.StringAlignment]::Center
    $sRect = New-Object System.Drawing.RectangleF(24, 24, 252, 252)
    $sg.DrawString("S", $sFont, $sBrushFg, $sRect, $sSf)
    $StoreListingPath = Join-Path $ProjectRoot "StoreListing_Icon_300x300.png"
    $StoreListingBmp.Save($StoreListingPath, [System.Drawing.Imaging.ImageFormat]::Png)
    $sFont.Dispose()
    $sg.Dispose()
    $StoreListingBmp.Dispose()
}

Generate-IconAssets $AssetsDir

# Build resources.pri if makepri.exe is available
if (Test-Path $MakePri) {
    $PriConfig = Join-Path $env:TEMP "smart_scheduler_priconfig.xml"
    & $MakePri createconfig /cf $PriConfig /dq "en-US_zh-CN" /o | Out-Null
    & $MakePri new /pr $MsixStage /cf $PriConfig /of (Join-Path $MsixStage "resources.pri") /o | Out-Null
    Remove-Item -Force $PriConfig -ErrorAction SilentlyContinue
}

Write-Host "[4/5] Packing MSIX via makeappx.exe ($Arch) ..." -ForegroundColor Cyan
$MsixOut = Join-Path $ProjectRoot "SmartScheduler_${PkgVersion}_${Arch}.msix"
& $MakeAppx pack /d $MsixStage /p $MsixOut /o | Out-Host
if ($LASTEXITCODE -ne 0) { throw "makeappx pack failed" }

Write-Host "[5/5] Managing digital signature ..." -ForegroundColor Cyan
$Subject = if ($StorePublisher) { $StorePublisher } else { "CN=8BE064BB-20F6-4B24-8FB4-41C94B229A53" }

if (-not $StoreBuild) {
    # Local dev self-signed certificate for direct sideloading
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
} else {
    Write-Host "Store build selected: Leaving package ready for Microsoft Store signing pipeline." -ForegroundColor Green
    $CerPath = "N/A (Microsoft Store signs package during ingestion)"
}

$StandaloneExeOut = Join-Path $ProjectRoot "smart-scheduler-$Arch.exe"
Copy-Item -Force $ExePath $StandaloneExeOut

$ExeSize = (Get-Item $ExePath).Length
$MsixSize = (Get-Item $MsixOut).Length
Write-Host ""
Write-Host "MSIX & Standalone EXE Build Complete ($Arch)!" -ForegroundColor Green
Write-Host "  Standalone:     $StandaloneExeOut ($ExeSize bytes)"
Write-Host "  MSIX Package:   $MsixOut ($MsixSize bytes)"
Write-Host "  Store Icon:     $(Join-Path $ProjectRoot "StoreListing_Icon_300x300.png")"
Write-Host "  Certificate:    $CerPath"
