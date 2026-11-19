param(
    [ValidateSet("x64", "arm64")][string]$Arch = $(if ($env:PROCESSOR_ARCHITECTURE -match "ARM64") { "arm64" } else { "x64" }),
    [string]$PackageName = "DerekLi.SmartScheduler"
)

$ErrorActionPreference = "Stop"
$ProjectRoot = Split-Path -Parent $MyInvocation.MyCommand.Path

# Resolve MSIX path (from Cargo.toml when in repo, or by scanning downloaded release files)
$CargoPath = Join-Path $ProjectRoot "Cargo.toml"
$MsixPath = $null
if (Test-Path $CargoPath) {
    $CargoToml = Get-Content $CargoPath -Raw
    if ($CargoToml -match 'version\s*=\s*"([^"]+)"') {
        $PkgVersion = $matches[1] + ".0"
        $MsixPath = Join-Path $ProjectRoot "SmartScheduler_${PkgVersion}_${Arch}.msix"
    }
}

if (-not $MsixPath) {
    $FoundMsix = Get-ChildItem -Path $ProjectRoot -Filter "SmartScheduler_*_${Arch}.msix" -ErrorAction SilentlyContinue |
        Sort-Object Name -Descending |
        Select-Object -First 1
    if ($FoundMsix) {
        $MsixPath = $FoundMsix.FullName
    }
}

$CerPath = Join-Path $ProjectRoot "SmartScheduler_Cert.cer"

if (-not $MsixPath -or -not (Test-Path $MsixPath)) {
    $BuildScript = Join-Path $ProjectRoot "build-msix.ps1"
    if ((Test-Path $CargoPath) -and (Test-Path $BuildScript)) {
        Write-Host "Package not found. Invoking build-msix.ps1 -Arch $Arch ..." -ForegroundColor Yellow
        & $BuildScript -Arch $Arch
        if ($PkgVersion) {
            $MsixPath = Join-Path $ProjectRoot "SmartScheduler_${PkgVersion}_${Arch}.msix"
        }
    } else {
        throw "MSIX package for $Arch (SmartScheduler_*_${Arch}.msix) not found in $ProjectRoot."
    }
}

if (-not (Test-Path $CerPath)) {
    throw "Certificate file $CerPath not found in $ProjectRoot."
}

# For a self-signed MSIX to install cleanly via double-click (App Installer) or Add-AppxPackage,
# its root certificate must be present in Cert:\LocalMachine\Root AND Cert:\LocalMachine\TrustedPeople.
$Cert = New-Object System.Security.Cryptography.X509Certificates.X509Certificate2($CerPath)
$InRoot = Get-ChildItem Cert:\LocalMachine\Root -ErrorAction SilentlyContinue | Where-Object { $_.Thumbprint -eq $Cert.Thumbprint }
$InTrusted = Get-ChildItem Cert:\LocalMachine\TrustedPeople -ErrorAction SilentlyContinue | Where-Object { $_.Thumbprint -eq $Cert.Thumbprint }

if ((-not $InRoot) -or (-not $InTrusted)) {
    Write-Host "Requesting Administrator privilege to trust certificate in LocalMachine\Root & LocalMachine\TrustedPeople..." -ForegroundColor Yellow
    $Cmd = "Import-Certificate -FilePath '$CerPath' -CertStoreLocation 'Cert:\LocalMachine\Root' | Out-Null; Import-Certificate -FilePath '$CerPath' -CertStoreLocation 'Cert:\LocalMachine\TrustedPeople' | Out-Null"
    Start-Process powershell.exe -Verb RunAs -ArgumentList "-NoProfile -ExecutionPolicy Bypass -Command `"$Cmd`"" -Wait
}

Write-Host "Installing $(Split-Path $MsixPath -Leaf) ($Arch) ..." -ForegroundColor Cyan
Stop-Process -Name "smart-scheduler" -Force -ErrorAction SilentlyContinue

# Back up existing MSIX LocalState\config.toml before removing same-version package (avoids 0x80073CFB)
$ExistingPkg = Get-AppxPackage -Name $PackageName | Select-Object -First 1
$SavedConfigContent = $null
if ($ExistingPkg) {
    $ExistingCfgPath = Join-Path $env:LOCALAPPDATA "Packages\$($ExistingPkg.PackageFamilyName)\LocalState\config.toml"
    if (Test-Path $ExistingCfgPath) {
        $SavedConfigContent = [System.IO.File]::ReadAllBytes($ExistingCfgPath)
    }
    Remove-AppxPackage -Package $ExistingPkg.PackageFullName -ErrorAction SilentlyContinue
}

Add-AppxPackage -Path $MsixPath -ForceApplicationShutdown

# Restore or pre-populate the MSIX LocalState config
$Pkg = Get-AppxPackage -Name $PackageName | Select-Object -First 1
if ($Pkg) {
    $LocalStateDir = Join-Path $env:LOCALAPPDATA "Packages\$($Pkg.PackageFamilyName)\LocalState"
    New-Item -ItemType Directory -Force -Path $LocalStateDir | Out-Null
    $TargetCfg = Join-Path $LocalStateDir "config.toml"
    $PortableCfg = Join-Path $ProjectRoot "target\release\config.toml"
    if ($SavedConfigContent -ne $null) {
        [System.IO.File]::WriteAllBytes($TargetCfg, $SavedConfigContent)
    } elseif ((Test-Path $PortableCfg) -and (-not (Test-Path $TargetCfg))) {
        Copy-Item -Force $PortableCfg $TargetCfg
    }
    Write-Host "Launching Smart Scheduler from MSIX package..." -ForegroundColor Green
    explorer.exe "shell:AppsFolder\$($Pkg.PackageFamilyName)!SmartScheduler"
    Start-Sleep -Seconds 1
    Get-ChildItem "HKCU:\Control Panel\NotifyIconSettings" -ErrorAction SilentlyContinue | ForEach-Object {
        $p = Get-ItemProperty $_.PSPath -ErrorAction SilentlyContinue
        if ($p.ExecutablePath -match 'SmartScheduler|smart-scheduler') {
            Set-ItemProperty -Path $_.PSPath -Name "IsPromoted" -Value 1 -Type DWord -ErrorAction SilentlyContinue
        }
    }
}

Write-Host "Smart Scheduler MSIX installed and running!" -ForegroundColor Green
