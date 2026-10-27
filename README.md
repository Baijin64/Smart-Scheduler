# Smart Scheduler

An ultra-lightweight, zero-dependency Windows CPU Affinity & FairShare dynamic background scheduler written in pure Rust. Native support for both **x86_64** and **ARM64** (Windows on ARM / Qualcomm Snapdragon X Elite).

## Key Highlights

- **Tiny Footprint**: `~240 KB` single binary, `~2.5 MB` Working Set RAM, `0.00%` steady-state CPU usage.
- **Single-Syscall Snapshot**: Uses `NtQuerySystemInformation(SystemProcessInformation)` with pre-allocated reusable buffers (`0` heap allocations per tick).
- **Hardware-Agnostic Topology Detection**: Queries `GetLogicalProcessorInformationEx` at startup to automatically detect:
  - **AMD Multi-CCD / 3D V-Cache** processors (automatically distinguishes 96MB V-Cache CCD vs 32MB Frequency CCD by L3 cache size).
  - **Intel Hybrid (P-Core + E-Core)** processors via `EfficiencyClass`.
  - **Single-CCD / Standard** processors and **SMT0 Physical Core** masks.
  - **ARM64 / WoA Multi-Cluster** topologies.
- **Zero-Touch Critical Process Immunity**: Never injects DLLs or hooks, and never opens handles to critical Windows shell/system processes (`explorer.exe`, `dwm.exe`, `csrss.exe`, `svchost.exe`, etc.).
- **Clean 8-Line Tray Menu & Per-Process Rules**:
  1. Configure current foreground window in 1 click (CPU Affinity, Priority Class, FairShare Exemption/Force, Persistent Enforce)
  2. Configure from **Top 12 Active Processes** (sorted by real-time CPU usage)
  3. Pick any `.exe` via isolated child-process native file dialog (`comdlg32.dll!GetOpenFileNameW` without bloating daemon RAM)
  4. Manage/modify/remove existing rules (`[rules]` supports exact names, `*`/`?` wildcards like `*-shipping.exe`, and full-path patterns like `C:\Games\*\game.exe`)
  5. Toggle CPU Affinity/Priority enforcement & FairShare dynamic background restraint
  6. 1-click Autostart toggle (`HKCU\...\Run` for portable builds + `windows.startupTask` for MSIX), automatic `config.toml` hot-reload, and Process Lasso `.json` / `.ini` compatible rule import via file picker
- **MSIX Packaging & Zero-Residue Uninstall**: Stores configuration in the MSIX package `LocalState` container and integrates with Windows Startup Apps (`windows.startupTask`).

## Build & Install (MSIX)

### 1. Build and Package

```powershell
# Default x64 build
.\build-msix.ps1

# ARM64 build (or package existing tested binary with -SkipCargoBuild)
.\build-msix.ps1 -Arch arm64
```

### 2. Install

Trust the local self-signed certificate and install the MSIX package (will prompt once for Administrator privilege to add the certificate to `LocalMachine\Root` and `LocalMachine\TrustedPeople`):

```powershell
# Automatically detects current system architecture (x64 / arm64)
.\install-msix.ps1

# Or explicitly target an architecture
.\install-msix.ps1 -Arch arm64
```

## Disclaimer & Trademarks

- **Inspiration**: Smart Scheduler is an independent, minimalist open-source project inspired by the scheduling philosophy of Process Lasso.
- **Clean-Slate Implementation**: This project is written entirely from scratch in pure Rust without using, reverse-engineering, or redistributing any proprietary code or binary assets from Bitsum LLC.
- **Trademark Notice**: *Process Lasso* and *ProBalance* are registered trademarks of Bitsum LLC. Smart Scheduler is not affiliated with, sponsored by, or endorsed by Bitsum LLC.
- **Notice & Takedown**: If you believe any content in this repository infringes upon your copyright or trademark rights, please open a GitHub Issue or contact `169961693+Baijin64@users.noreply.github.com`, and it will be addressed promptly.

## License

This project is licensed under the [MIT License](LICENSE).
