use std::fs;
use std::path::{Path, PathBuf};
use crate::win32::{
    set_windows_gpu_preference, to_wide, GetCurrentPackageFamilyName, GetEnvironmentVariableW,
    GetModuleFileNameW, ABOVE_NORMAL_PRIORITY_CLASS, BELOW_NORMAL_PRIORITY_CLASS, DWORD,
    ERROR_SUCCESS, HIGH_PRIORITY_CLASS, IDLE_PRIORITY_CLASS, NORMAL_PRIORITY_CLASS, NULL,
};

/// How FairShare treats a process covered by a rule.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FairShareMode {
    /// Normal FairShare logic (a rule that sets an explicit priority implies `Exempt`).
    Default,
    /// Never restrain this process.
    Exempt,
    /// Restrain whenever the CPU thresholds are exceeded, ignoring foreground/service/exclusion checks.
    Force,
}

/// One per-process rule. A rule may set any combination of affinity, priority and FairShare mode.
#[derive(Clone, Debug)]
pub struct Rule {
    pub pattern: String,              // lowercase; exe name, stem, glob (`*`/`?`) or full-path glob
    pub stem: String,                 // pattern without `.exe` (plain-name patterns only)
    pub is_glob: bool,                // contains `*` or `?`
    pub match_path: bool,             // contains a path separator or drive colon -> match full image path
    pub affinity_str: Option<String>, // e.g., "0-15", "4-15", "0x00FF"
    pub affinity_mask: Option<usize>, // bitmask of `affinity_str`
    pub priority: Option<DWORD>,      // Windows priority class
    pub fairshare: FairShareMode,
    pub delay_ms: u64,                // wait this long after process start before applying
    pub persist: bool,                // re-verify periodically (some programs reset their own settings)
}

#[derive(Clone, Debug)]
pub struct GpuRule {
    pub exe_path: String,
    pub pref_code: u8, // 1 = 节能 (Power Saving), 2 = 高性能 (High Performance)
}

#[derive(Clone, Debug)]
pub struct FairShareConfig {
    pub enabled: bool,
    pub total_cpu_threshold_pct: f32,
    pub process_cpu_threshold_pct: f32,
    pub time_over_quota_ms: u64,
    pub process_restore_pct: f32,
    pub min_restraint_ms: u64,
    pub exclude_foreground: bool,
    pub exclude_foreground_children: bool,
    pub exclude_services: bool,
    pub tame_only_normal: bool,
    pub exclusions: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub interval_ms: u32,
    pub auto_elevate: bool,
    pub affinity_enabled: bool,
    pub enforce_affinity_every_ticks: u32,
    pub language: String,
    pub fairshare: FairShareConfig,
    pub rules: Vec<Rule>,
    pub gpu_rules: Vec<GpuRule>,
}

/// Hard-coded critical Windows shell & kernel processes that should NEVER be opened or touched
/// by FairShare or background polling unless the user explicitly created an affinity rule for them.
pub const CRITICAL_SYSTEM_PROCESSES: &[&str] = &[
    "system",
    "registry",
    "smss.exe",
    "csrss.exe",
    "wininit.exe",
    "services.exe",
    "lsass.exe",
    "lsm.exe",
    "winlogon.exe",
    "fontdrvhost.exe",
    "dwm.exe",
    "explorer.exe",
    "sihost.exe",
    "taskhostw.exe",
    "shellexperiencehost.exe",
    "startmenuexperiencehost.exe",
    "searchhost.exe",
    "textinputhost.exe",
    "ctfmon.exe",
    "runtimebroker.exe",
    "applicationframehost.exe",
    "dllhost.exe",
    "svchost.exe",
    "spoolsv.exe",
    "wmiprvse.exe",
    "wudfhost.exe",
    "msmpeng.exe",
    "nissrv.exe",
    "securityhealthservice.exe",
    "securityhealthsystray.exe",
    "memory compression",
    "smart-scheduler.exe",
    "processlasso.exe",
    "processgovernor.exe",
];

pub fn is_critical_system_process(name_lower: &str) -> bool {
    CRITICAL_SYSTEM_PROCESSES.iter().any(|&c| c == name_lower)
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            interval_ms: 1000,
            auto_elevate: true,
            affinity_enabled: true,
            enforce_affinity_every_ticks: 5,
            language: "auto".to_string(),
            fairshare: FairShareConfig {
                enabled: true,
                total_cpu_threshold_pct: 7.0,
                process_cpu_threshold_pct: 2.0,
                time_over_quota_ms: 900,
                process_restore_pct: 1.0,
                min_restraint_ms: 4200,
                exclude_foreground: true,
                exclude_foreground_children: true,
                exclude_services: true,
                tame_only_normal: true,
                exclusions: Vec::new(),
            },
            rules: Vec::new(),
            gpu_rules: Vec::new(),
        }
    }
}

impl Rule {
    /// Rule with no settings yet for `pattern`; `None` if the pattern is blank.
    pub fn base(pattern: &str) -> Option<Self> {
        let mut lower = pattern.trim().to_ascii_lowercase();
        if lower.is_empty() {
            return None;
        }
        let match_path = lower.contains(['\\', '/', ':']);
        if match_path {
            lower = lower.replace('/', "\\");
        }
        let is_glob = lower.contains(['*', '?']);
        let stem = lower.strip_suffix(".exe").unwrap_or(&lower).to_string();
        Some(Self {
            pattern: lower,
            stem,
            is_glob,
            match_path,
            affinity_str: None,
            affinity_mask: None,
            priority: None,
            fairshare: FairShareMode::Default,
            delay_ms: 0,
            persist: true,
        })
    }

    pub fn with_affinity(pattern: &str, spec: &str, delay_ms: u64) -> Option<Self> {
        let mut r = Self::base(pattern)?;
        if !r.set_affinity(spec) {
            return None;
        }
        r.delay_ms = delay_ms;
        Some(r)
    }

    pub fn set_affinity(&mut self, spec: &str) -> bool {
        match parse_cpu_spec(spec) {
            Some(mask) => {
                self.affinity_mask = Some(mask);
                self.affinity_str = Some(spec.trim().to_string());
                true
            }
            None => false,
        }
    }

    pub fn has_settings(&self) -> bool {
        self.affinity_mask.is_some()
            || self.priority.is_some()
            || self.fairshare != FairShareMode::Default
    }

    /// Parses a config value such as `0-15 prio=high fs=exempt delay=2s persist=off`.
    /// The legacy forms `pb=exempt` and `0-15@500ms` are still accepted.
    pub fn parse(pattern: &str, value: &str) -> Option<Self> {
        let mut r = Self::base(pattern)?;
        for tok in value.split_whitespace() {
            if let Some((k, v)) = tok.split_once('=') {
                let v = v.to_ascii_lowercase();
                match k.to_ascii_lowercase().as_str() {
                    "prio" | "priority" => r.priority = priority_from_str(&v),
                    "fs" | "fairshare" => {
                        let mode = match v.as_str() {
                            "exempt" | "skip" => FairShareMode::Exempt,
                            "force" => FairShareMode::Force,
                            _ => FairShareMode::Default,
                        };
                        r.fairshare = mode;
                    }
                    "delay" => r.delay_ms = parse_duration_ms(&v),
                    "persist" => r.persist = !matches!(v.as_str(), "off" | "false" | "no" | "0"),
                    _ => {}
                }
            } else {
                let (spec, delay) = match tok.split_once('@') {
                    Some((s, d)) => (s, Some(parse_duration_ms(d))),
                    None => (tok, None),
                };
                if r.set_affinity(spec) {
                    if let Some(d) = delay {
                        r.delay_ms = d;
                    }
                }
            }
        }
        r.has_settings().then_some(r)
    }

    pub fn to_config_value(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if let Some(a) = &self.affinity_str {
            parts.push(a.clone());
        }
        if let Some(p) = self.priority {
            parts.push(format!("prio={}", priority_to_str(p)));
        }
        match self.fairshare {
            FairShareMode::Exempt => parts.push("fs=exempt".into()),
            FairShareMode::Force => parts.push("fs=force".into()),
            FairShareMode::Default => {}
        }
        if self.delay_ms > 0 {
            parts.push(format!("delay={}ms", self.delay_ms));
        }
        if !self.persist {
            parts.push("persist=off".into());
        }
        parts.join(" ")
    }

    /// Short human-readable description for menus and logs with localization support.
    pub fn summary_localized(&self, is_zh: bool) -> String {
        let mut parts: Vec<String> = Vec::new();
        if let Some(a) = &self.affinity_str {
            parts.push(if is_zh {
                format!("核心 {}", a)
            } else {
                format!("Cores {}", a)
            });
        }
        if let Some(p) = self.priority {
            parts.push(if is_zh {
                format!("优先级 {}", priority_to_str(p))
            } else {
                format!("Priority {}", priority_to_str(p))
            });
        }
        match self.fairshare {
            FairShareMode::Exempt => parts.push(if is_zh { "FS豁免".into() } else { "FS Exempt".into() }),
            FairShareMode::Force => parts.push(if is_zh { "FS强力".into() } else { "FS Force".into() }),
            FairShareMode::Default => {}
        }
        parts.join(" · ")
    }

    /// Short human-readable description for menus and logs.
    pub fn summary(&self) -> String {
        self.summary_localized(true)
    }

    /// `path_lower` is only consulted by full-path rules (`None` = unknown -> no match).
    pub fn matches(&self, name_lower: &str, path_lower: Option<&str>) -> bool {
        if self.match_path {
            return path_lower.map_or(false, |p| glob_match(&self.pattern, p));
        }
        if self.is_glob {
            return glob_match(&self.pattern, name_lower);
        }
        name_lower == self.pattern
            || name_lower.strip_suffix(".exe").unwrap_or(name_lower) == self.stem
    }

    /// True for a plain-name rule (no wildcard / path) that targets exactly `name_lower`.
    pub fn is_exact_for(&self, name_lower: &str) -> bool {
        !self.is_glob && !self.match_path && self.matches(name_lower, None)
    }
}

/// Case-sensitive glob (`*` = any run, `?` = one byte); callers pass lowercase text.
pub fn glob_match(pat: &str, text: &str) -> bool {
    let (p, t) = (pat.as_bytes(), text.as_bytes());
    let (mut pi, mut ti) = (0usize, 0usize);
    let (mut star, mut mark) = (usize::MAX, 0usize);
    while ti < t.len() {
        if pi < p.len() && (p[pi] == b'?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == b'*' {
            star = pi;
            mark = ti;
            pi += 1;
        } else if star != usize::MAX {
            pi = star + 1;
            mark += 1;
            ti = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == b'*' {
        pi += 1;
    }
    pi == p.len()
}

fn priority_from_str(s: &str) -> Option<DWORD> {
    match s {
        "idle" | "low" => Some(IDLE_PRIORITY_CLASS),
        "below" | "belownormal" => Some(BELOW_NORMAL_PRIORITY_CLASS),
        "normal" => Some(NORMAL_PRIORITY_CLASS),
        "above" | "abovenormal" => Some(ABOVE_NORMAL_PRIORITY_CLASS),
        "high" => Some(HIGH_PRIORITY_CLASS),
        _ => None,
    }
}

fn priority_to_str(p: DWORD) -> &'static str {
    match p {
        IDLE_PRIORITY_CLASS => "idle",
        BELOW_NORMAL_PRIORITY_CLASS => "below",
        ABOVE_NORMAL_PRIORITY_CLASS => "above",
        HIGH_PRIORITY_CLASS => "high",
        _ => "normal",
    }
}

/// `"500ms"` -> 500, `"2s"` / `"2"` -> 2000.
fn parse_duration_ms(s: &str) -> u64 {
    let s = s.trim();
    if let Some(n) = s.strip_suffix("ms") {
        n.trim().parse().unwrap_or(0)
    } else {
        s.trim_end_matches('s').trim().parse::<u64>().map(|n| n * 1000).unwrap_or(0)
    }
}

/// Parses CPU specification like "0-15", "4-15", "16-31", "0,2,4-7", or hex "0xFFFF"
pub fn parse_cpu_spec(spec: &str) -> Option<usize> {
    let s = spec.trim();
    if s.is_empty() {
        return None;
    }
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        return usize::from_str_radix(hex, 16).ok().filter(|&m| m != 0);
    }
    let mut mask: usize = 0;
    for part in s.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some((start_s, end_s)) = part.split_once('-') {
            let start: u32 = start_s.trim().parse().ok()?;
            let end: u32 = end_s.trim().parse().ok()?;
            if start > end || end >= usize::BITS {
                return None;
            }
            for bit in start..=end {
                mask |= 1usize << bit;
            }
        } else {
            let bit: u32 = part.parse().ok()?;
            if bit >= usize::BITS {
                return None;
            }
            mask |= 1usize << bit;
        }
    }
    if mask != 0 {
        Some(mask)
    } else {
        None
    }
}

pub fn resolve_config_path() -> (PathBuf, bool) {
    unsafe {
        let mut len: u32 = 0;
        let _ = GetCurrentPackageFamilyName(&mut len, std::ptr::null_mut());
        if len > 0 {
            let mut fam_buf = vec![0u16; len as usize];
            if GetCurrentPackageFamilyName(&mut len, fam_buf.as_mut_ptr()) == ERROR_SUCCESS {
                let family_name =
                    String::from_utf16_lossy(&fam_buf[..(len as usize).saturating_sub(1)]);
                let mut local_app_data = vec![0u16; 512];
                let env_name = to_wide("LOCALAPPDATA");
                let env_len =
                    GetEnvironmentVariableW(env_name.as_ptr(), local_app_data.as_mut_ptr(), 512);
                if env_len > 0 {
                    let base = String::from_utf16_lossy(&local_app_data[..env_len as usize]);
                    let mut dir = PathBuf::from(base);
                    dir.push("Packages");
                    dir.push(family_name);
                    dir.push("LocalState");
                    let _ = fs::create_dir_all(&dir);
                    dir.push("config.toml");
                    return (dir, true);
                }
            }
        }

        let mut exe_buf = vec![0u16; 1024];
        let n = GetModuleFileNameW(NULL as _, exe_buf.as_mut_ptr(), 1024);
        let mut path = if n > 0 {
            PathBuf::from(String::from_utf16_lossy(&exe_buf[..n as usize]))
        } else {
            PathBuf::from("smart-scheduler.exe")
        };
        path.set_file_name("config.toml");
        (path, false)
    }
}

/// Reads text file supporting UTF-8, UTF-8 BOM, and UTF-16LE (used by Process Lasso INI exports)
pub fn read_text_auto_encoding(path: &Path) -> std::io::Result<String> {
    let bytes = fs::read(path)?;
    if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
        // UTF-16LE with BOM
        let u16_slice: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        Ok(String::from_utf16_lossy(&u16_slice))
    } else if bytes.len() >= 4 && bytes[1] == 0 && bytes[3] == 0 {
        // UTF-16LE without BOM
        let u16_slice: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        Ok(String::from_utf16_lossy(&u16_slice))
    } else if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        Ok(String::from_utf8_lossy(&bytes[3..]).into_owned())
    } else {
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }
}

impl AppConfig {
    /// Highest-priority rule matching by name only (exact rules always take precedence over globs).
    /// Wildcard rules never apply to critical system processes.
    pub fn find_rule_by_name(&self, name_lower: &str) -> Option<&Rule> {
        let crit = is_critical_system_process(name_lower);
        self.rules
            .iter()
            .find(|r| r.is_exact_for(name_lower))
            .or_else(|| {
                self.rules
                    .iter()
                    .find(|r| !(crit && r.is_glob) && !r.match_path && r.matches(name_lower, None))
            })
    }

    pub fn load_or_create() -> (Self, PathBuf, bool) {
        let (path, is_msix) = resolve_config_path();
        if let Ok(content) = fs::read_to_string(&path) {
            let cfg = Self::parse(&content);
            cfg.apply_gpu_rules();
            (cfg, path, is_msix)
        } else {
            let cfg = Self::default();
            cfg.apply_gpu_rules();
            let _ = cfg.save(&path);
            (cfg, path, is_msix)
        }
    }

    pub fn apply_gpu_rules(&self) -> usize {
        let mut applied = 0;
        for g in &self.gpu_rules {
            if set_windows_gpu_preference(&g.exe_path, g.pref_code) {
                applied += 1;
            }
        }
        applied
    }

    /// Imports rules from Process Lasso exported `.json` (e.g. `9950x-3D.json`) or `.ini` (`9950x-3d.ini`)
    pub fn import_process_lasso_file(&mut self, path: &Path) -> Result<(usize, usize), String> {
        let content = read_text_auto_encoding(path).map_err(|e| e.to_string())?;
        let trimmed = content.trim_start();
        if trimmed.starts_with('{') {
            self.import_pl_json(trimmed)
        } else {
            self.import_pl_ini(trimmed)
        }
    }

    fn import_pl_json(&mut self, json: &str) -> Result<(usize, usize), String> {
        let mut new_rules: Vec<Rule> = Vec::new();
        let mut new_gpu: Vec<GpuRule> = Vec::new();

        // Parse objects inside "ruleSets": [ { ... }, { ... } ]
        if let Some(rs_idx) = json.find("\"ruleSets\"") {
            let rest = &json[rs_idx..];
            if let Some(arr_start) = rest.find('[') {
                let arr_body = &rest[arr_start + 1..];
                let mut depth = 0i32;
                let mut obj_start = None;
                for (i, ch) in arr_body.char_indices() {
                    match ch {
                        '{' => {
                            if depth == 0 {
                                obj_start = Some(i + 1);
                            }
                            depth += 1;
                        }
                        '}' => {
                            depth -= 1;
                            if depth == 0 {
                                if let Some(start) = obj_start {
                                    let obj_str = &arr_body[start..i];
                                    let proc_name = extract_json_str(obj_str, "processName");
                                    let cpu_aff = extract_json_str(obj_str, "cpuAffinity");
                                    let delay_s = extract_json_num(obj_str, "cpuAffinityDelay").unwrap_or(0);
                                    let gpu_pref = extract_json_str(obj_str, "gpuAdapterPreference");

                                    if let (Some(name), Some(aff)) = (&proc_name, &cpu_aff) {
                                        if let Some(r) = Rule::with_affinity(name, aff, delay_s * 1000) {
                                            new_rules.push(r);
                                        }
                                    }
                                    if let (Some(name), Some(gpu)) = (&proc_name, &gpu_pref) {
                                        let code = if gpu.contains("高性能") || gpu.eq_ignore_ascii_case("HighPerformance") || gpu == "2" {
                                            2
                                        } else if gpu.contains("节能") || gpu.eq_ignore_ascii_case("MinimumPower") || gpu == "1" {
                                            1
                                        } else {
                                            0
                                        };
                                        if code > 0 {
                                            new_gpu.push(GpuRule {
                                                exe_path: name.replace("\\\\", "\\"),
                                                pref_code: code,
                                            });
                                        }
                                    }
                                }
                                obj_start = None;
                            }
                        }
                        ']' if depth == 0 => break,
                        _ => {}
                    }
                }
            }
        }

        if new_rules.is_empty() && new_gpu.is_empty() {
            return Err("未在 JSON 中找到有效的 ruleSets 规则".into());
        }
        let aff_count = new_rules.len();
        let gpu_count = new_gpu.len();
        if !new_rules.is_empty() {
            self.rules = new_rules;
        }
        if !new_gpu.is_empty() {
            self.gpu_rules = new_gpu;
            self.apply_gpu_rules();
        }
        Ok((aff_count, gpu_count))
    }

    fn import_pl_ini(&mut self, ini: &str) -> Result<(usize, usize), String> {
        let mut aff_count = 0usize;
        let mut gpu_count = 0usize;

        for raw_line in ini.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('[') || line.starts_with(';') {
                continue;
            }
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            let k = k.trim();
            let v = v.trim();
            match k {
                "OocOn" => if let Ok(b) = v.parse::<bool>() { self.fairshare.enabled = b; },
                "TotalProcessorUsageBeforeRestraint" => if let Ok(f) = v.parse::<f32>() { self.fairshare.total_cpu_threshold_pct = f; },
                "PerProcessUsageBeforeRestraint" => if let Ok(f) = v.parse::<f32>() { self.fairshare.process_cpu_threshold_pct = f; },
                "TimeOverQuotaBeforeRestraint" => if let Ok(n) = v.parse::<u64>() { self.fairshare.time_over_quota_ms = n; },
                "PerProcessUsageForRestore" => if let Ok(f) = v.parse::<f32>() { self.fairshare.process_restore_pct = f; },
                "MinimumTimeOfRestraint" => if let Ok(n) = v.parse::<u64>() { self.fairshare.min_restraint_ms = n; },
                "ExcludeChildrenOfForeground" => if let Ok(b) = v.parse::<bool>() { self.fairshare.exclude_foreground_children = b; },
                "ExcludeServices" => if let Ok(b) = v.parse::<bool>() { self.fairshare.exclude_services = b; },
                "TameOnlyNormal" => if let Ok(b) = v.parse::<bool>() { self.fairshare.tame_only_normal = b; },
                "DefaultAffinitiesEx" if !v.is_empty() => {
                    // Format: proc1,delay1,cores1,proc2,delay2,cores2,...
                    let parts: Vec<&str> = v.split(',').collect();
                    let mut parsed_rules = Vec::new();
                    for chunk in parts.chunks_exact(3) {
                        let name = chunk[0].trim();
                        let delay_s: u64 = chunk[1].trim().parse().unwrap_or(0);
                        let cores = chunk[2].trim();
                        if let Some(r) = Rule::with_affinity(name, cores, delay_s * 1000) {
                            parsed_rules.push(r);
                        }
                    }
                    if !parsed_rules.is_empty() {
                        aff_count = parsed_rules.len();
                        self.rules = parsed_rules;
                    }
                }
                "DefaultGPUAdapterPreferences" if !v.is_empty() => {
                    let parts: Vec<&str> = v.split(',').collect();
                    let mut parsed_gpu = Vec::new();
                    for chunk in parts.chunks_exact(2) {
                        let exe = chunk[0].trim();
                        let code: u8 = chunk[1].trim().parse().unwrap_or(0);
                        if (code == 1 || code == 2) && !exe.is_empty() {
                            parsed_gpu.push(GpuRule {
                                exe_path: exe.to_string(),
                                pref_code: code,
                            });
                        }
                    }
                    if !parsed_gpu.is_empty() {
                        gpu_count = parsed_gpu.len();
                        self.gpu_rules = parsed_gpu;
                        self.apply_gpu_rules();
                    }
                }
                _ => {}
            }
        }

        if aff_count == 0 && gpu_count == 0 {
            return Err("未在 INI 中找到 DefaultAffinitiesEx 或 GPU 规则".into());
        }
        Ok((aff_count, gpu_count))
    }

    pub fn save(&self, path: &PathBuf) -> std::io::Result<()> {
        let mut out = String::with_capacity(2048);
        out.push_str("# Smart Scheduler 配置文件 (保存后自动生效，无需重启)\n");
        out.push_str("# [rules] 每行一条: \"匹配模式\" = \"设置...\"，自上而下首个命中的规则生效\n");
        out.push_str("#   匹配模式: 进程名 (chrome.exe / chrome)、通配符 (chrome*, *-shipping.exe)、\n");
        out.push_str("#             或完整路径通配符 (c:\\games\\*\\game.exe，含 \\ 或 : 时按完整路径匹配)\n");
        out.push_str("#   设置 (空格分隔，任意组合):\n");
        out.push_str("#     核心范围       0-15 | 4-15 | 0,2,4-7 | 0xFF00   (CPU 亲和性)\n");
        out.push_str("#     prio=          idle | below | normal | above | high   (进程优先级)\n");
        out.push_str("#     fs=            exempt (免于 FairShare 调度) | force (超阈值即调度，忽略前台/服务/排除)\n");
        out.push_str("#     delay=         启动后延迟多久再应用，如 500ms / 2s\n");
        out.push_str("#     persist=off    关闭定期复查 (默认每 enforce_affinity_every_ticks 个周期复查一次)\n");
        out.push_str("#   示例: \"game.exe\" = \"0-7 prio=high fs=exempt\"\n\n");
        out.push_str("[general]\n");
        out.push_str(&format!("interval_ms = {}\n", self.interval_ms));
        out.push_str(&format!("auto_elevate = {}\n", self.auto_elevate));
        out.push_str(&format!("affinity_enabled = {}\n", self.affinity_enabled));
        out.push_str(&format!("enforce_affinity_every_ticks = {}\n", self.enforce_affinity_every_ticks));
        out.push_str(&format!("language = \"{}\"\n\n", self.language));

        out.push_str("[fairshare]\n");
        out.push_str(&format!("enabled = {}\n", self.fairshare.enabled));
        out.push_str(&format!("total_cpu_threshold_pct = {:.1}\n", self.fairshare.total_cpu_threshold_pct));
        out.push_str(&format!("process_cpu_threshold_pct = {:.1}\n", self.fairshare.process_cpu_threshold_pct));
        out.push_str(&format!("time_over_quota_ms = {}\n", self.fairshare.time_over_quota_ms));
        out.push_str(&format!("process_restore_pct = {:.1}\n", self.fairshare.process_restore_pct));
        out.push_str(&format!("min_restraint_ms = {}\n", self.fairshare.min_restraint_ms));
        out.push_str(&format!("exclude_foreground = {}\n", self.fairshare.exclude_foreground));
        out.push_str(&format!("exclude_foreground_children = {}\n", self.fairshare.exclude_foreground_children));
        out.push_str(&format!("exclude_services = {}\n", self.fairshare.exclude_services));
        out.push_str(&format!("tame_only_normal = {}\n", self.fairshare.tame_only_normal));
        out.push_str(&format!("exclusions = \"{}\"\n\n", self.fairshare.exclusions.join(",")));

        out.push_str("[rules]\n");
        for rule in &self.rules {
            out.push_str(&format!("\"{}\" = \"{}\"\n", rule.pattern, rule.to_config_value()));
        }

        if !self.gpu_rules.is_empty() {
            out.push_str("\n[gpu_preferences]\n");
            out.push_str("# 1 = 节能 (核显), 2 = 高性能 (独显)\n");
            for g in &self.gpu_rules {
                out.push_str(&format!("\"{}\" = \"{}\"\n", g.exe_path, g.pref_code));
            }
        }

        fs::write(path, out)
    }

    pub fn parse(content: &str) -> Self {
        let mut cfg = Self::default();
        let mut section = "";
        let mut parsed_rules: Vec<Rule> = Vec::new();
        let mut parsed_gpu: Vec<GpuRule> = Vec::new();
        let mut saw_affinity_section = false;
        let mut saw_gpu_section = false;

        for raw_line in content.lines() {
            let line = strip_comment(raw_line).trim();
            if line.is_empty() {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                section = &line[1..line.len() - 1];
                if section == "rules" || section == "affinity_rules" {
                    saw_affinity_section = true;
                } else if section == "gpu_preferences" {
                    saw_gpu_section = true;
                }
                continue;
            }
            let Some((k_raw, v_raw)) = split_key_value(line) else {
                continue;
            };
            let k = k_raw.trim().trim_matches('"');
            let v = v_raw.trim().trim_matches('"');

            match section {
                "general" => match k {
                    "interval_ms" => if let Ok(n) = v.parse::<u32>() { cfg.interval_ms = n.clamp(250, 10000); },
                    "auto_elevate" => if let Ok(b) = v.parse::<bool>() { cfg.auto_elevate = b; },
                    "affinity_enabled" => if let Ok(b) = v.parse::<bool>() { cfg.affinity_enabled = b; },
                    "enforce_affinity_every_ticks" => if let Ok(n) = v.parse::<u32>() { cfg.enforce_affinity_every_ticks = n; },
                    "language" => cfg.language = v.to_string(),
                    _ => {}
                },
                "fairshare" => match k {
                    "enabled" => if let Ok(b) = v.parse::<bool>() { cfg.fairshare.enabled = b; },
                    "total_cpu_threshold_pct" => if let Ok(f) = v.parse::<f32>() { cfg.fairshare.total_cpu_threshold_pct = f; },
                    "process_cpu_threshold_pct" => if let Ok(f) = v.parse::<f32>() { cfg.fairshare.process_cpu_threshold_pct = f; },
                    "time_over_quota_ms" => if let Ok(n) = v.parse::<u64>() { cfg.fairshare.time_over_quota_ms = n; },
                    "process_restore_pct" => if let Ok(f) = v.parse::<f32>() { cfg.fairshare.process_restore_pct = f; },
                    "min_restraint_ms" => if let Ok(n) = v.parse::<u64>() { cfg.fairshare.min_restraint_ms = n; },
                    "exclude_foreground" => if let Ok(b) = v.parse::<bool>() { cfg.fairshare.exclude_foreground = b; },
                    "exclude_foreground_children" => if let Ok(b) = v.parse::<bool>() { cfg.fairshare.exclude_foreground_children = b; },
                    "exclude_services" => if let Ok(b) = v.parse::<bool>() { cfg.fairshare.exclude_services = b; },
                    "tame_only_normal" => if let Ok(b) = v.parse::<bool>() { cfg.fairshare.tame_only_normal = b; },
                    "exclusions" => {
                        cfg.fairshare.exclusions = v
                            .split(',')
                            .map(|s| s.trim().to_ascii_lowercase())
                            .filter(|s| !s.is_empty())
                            .collect();
                    }
                    _ => {}
                },
                "rules" | "affinity_rules" => {
                    if let Some(rule) = Rule::parse(k, v) {
                        parsed_rules.push(rule);
                    }
                }
                "gpu_preferences" => {
                    if let Ok(code) = v.parse::<u8>() {
                        if code == 1 || code == 2 {
                            parsed_gpu.push(GpuRule {
                                exe_path: k.to_string(),
                                pref_code: code,
                            });
                        }
                    }
                }
                _ => {}
            }
        }

        if saw_affinity_section {
            cfg.rules = parsed_rules;
        }
        if saw_gpu_section {
            cfg.gpu_rules = parsed_gpu;
        }
        cfg
    }
}

/// Cuts a `#` comment, ignoring `#` inside double quotes (paths may contain it).
fn strip_comment(line: &str) -> &str {
    let mut in_quote = false;
    for (i, c) in line.char_indices() {
        match c {
            '"' => in_quote = !in_quote,
            '#' if !in_quote => return &line[..i],
            _ => {}
        }
    }
    line
}

/// Splits at the first `=` outside double quotes.
fn split_key_value(line: &str) -> Option<(&str, &str)> {
    let mut in_quote = false;
    for (i, c) in line.char_indices() {
        match c {
            '"' => in_quote = !in_quote,
            '=' if !in_quote => return Some((&line[..i], &line[i + 1..])),
            _ => {}
        }
    }
    None
}

fn extract_json_str(obj: &str, key: &str) -> Option<String> {
    let pat = format!("\"{}\"", key);
    let idx = obj.find(&pat)?;
    let after_key = &obj[idx + pat.len()..];
    let colon = after_key.find(':')?;
    let after_colon = after_key[colon + 1..].trim_start();
    if !after_colon.starts_with('"') {
        return None;
    }
    let body = &after_colon[1..];
    let mut out = String::new();
    let mut chars = body.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(esc) = chars.next() {
                out.push(esc);
            }
        } else if c == '"' {
            return Some(out);
        } else {
            out.push(c);
        }
    }
    None
}

fn extract_json_num(obj: &str, key: &str) -> Option<u64> {
    let pat = format!("\"{}\"", key);
    let idx = obj.find(&pat)?;
    let after_key = &obj[idx + pat.len()..];
    let colon = after_key.find(':')?;
    let after_colon = after_key[colon + 1..].trim_start();
    let digits: String = after_colon
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_glob_and_path_matching() {
        let r_exact = Rule::parse("StarRail.exe", "0-7").unwrap();
        assert!(r_exact.matches("starrail.exe", None));
        assert!(r_exact.matches("starrail", None));
        assert!(!r_exact.matches("starrail_launcher.exe", None));

        let r_glob = Rule::parse("*-Shipping.exe", "0-15 prio=high fs=exempt").unwrap();
        assert!(r_glob.is_glob);
        assert!(r_glob.matches("client-win64-shipping.exe", None));
        assert!(!r_glob.matches("explorer.exe", None));

        let r_path = Rule::parse("C:/Games/*/game.exe", "4-15 delay=2s persist=off").unwrap();
        assert!(r_path.match_path);
        assert!(!r_path.matches("game.exe", None));
        assert!(r_path.matches("game.exe", Some("c:\\games\\demo\\game.exe")));
        assert!(!r_path.matches("game.exe", Some("d:\\other\\game.exe")));
    }

    #[test]
    fn test_rule_roundtrip_and_non_affinity_rule() {
        let r1 = Rule::parse("chrome*", "prio=below fs=force").unwrap();
        assert_eq!(r1.affinity_mask, None);
        assert_eq!(r1.priority, Some(BELOW_NORMAL_PRIORITY_CLASS));
        assert_eq!(r1.fairshare, FairShareMode::Force);
        assert_eq!(r1.to_config_value(), "prio=below fs=force");

        let r2 = Rule::parse("app.exe", "0-7@1500ms prio=above fs=exempt persist=off").unwrap();
        assert_eq!(r2.affinity_mask, Some(0xFF));
        assert_eq!(r2.delay_ms, 1500);
        assert_eq!(r2.priority, Some(ABOVE_NORMAL_PRIORITY_CLASS));
        assert_eq!(r2.fairshare, FairShareMode::Exempt);
        assert!(!r2.persist);

        let serialized = format!("[rules]\n\"{}\" = \"{}\"\n", r2.pattern, r2.to_config_value());
        let cfg = AppConfig::parse(&serialized);
        assert_eq!(cfg.rules.len(), 1);
        assert_eq!(cfg.rules[0].affinity_mask, Some(0xFF));
        assert_eq!(cfg.rules[0].priority, Some(ABOVE_NORMAL_PRIORITY_CLASS));
        assert_eq!(cfg.rules[0].fairshare, FairShareMode::Exempt);
        assert_eq!(cfg.rules[0].delay_ms, 1500);
        assert!(!cfg.rules[0].persist);
    }
}

