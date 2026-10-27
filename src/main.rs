#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod engine;
mod i18n;
mod win32;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use config::{AppConfig, FairShareMode, Rule};
use engine::Engine;
use i18n::Language;
use win32::*;

const TIMER_ID_TICK: usize = 1;
const TOP_ACTIVE_LIMIT: usize = 12;
const MAX_PRESETS: usize = 8;

// Core Tray Menu Commands
const CMD_TOGGLE_AFFINITY: usize = 1001;
const CMD_TOGGLE_FAIRSHARE: usize = 1002;
const CMD_TOGGLE_AUTO_ELEVATE: usize = 1003;
const CMD_OPEN_CONFIG: usize = 1004;
const CMD_RELOAD_CONFIG: usize = 1005;
const CMD_ELEVATE_NOW: usize = 1006;
const CMD_TOGGLE_AUTOSTART: usize = 1007;
const CMD_IMPORT_PL_FILE: usize = 1008;
const CMD_STARTUP_SETTINGS: usize = 1009;
const CMD_LANG_AUTO: usize = 1030;
const CMD_LANG_ZH: usize = 1031;
const CMD_LANG_EN: usize = 1032;
const CMD_EXIT: usize = 1099;

// Per-Process Submenu Action Slots (0..STRIDE)
const SLOT_AFFINITY_ALL: usize = MAX_PRESETS; // 8: Clear affinity (restore all cores)
const SLOT_PRIO_NONE: usize = 9;
const SLOT_PRIO_HIGH: usize = 10;
const SLOT_PRIO_ABOVE: usize = 11;
const SLOT_PRIO_NORMAL: usize = 12;
const SLOT_PRIO_BELOW: usize = 13;
const SLOT_PRIO_IDLE: usize = 14;
const SLOT_FS_DEFAULT: usize = 15;
const SLOT_FS_EXEMPT: usize = 16;
const SLOT_FS_FORCE: usize = 17;
const SLOT_TOGGLE_PERSIST: usize = 18;
const SLOT_REMOVE_RULE: usize = 19;
const STRIDE: usize = 20;


// File Picker Extra Templates (beyond 0..MAX_PRESETS)
const PICK_EXTRA_HIGH_EXEMPT: usize = MAX_PRESETS;
const PICK_EXTRA_EXEMPT_ONLY: usize = MAX_PRESETS + 1;
const PICK_EXTRA_FORCE_LOW: usize = MAX_PRESETS + 2;
const PICK_TOTAL_SLOTS: usize = MAX_PRESETS + 3;

const CMD_FG_BASE: usize = 1100;     // 1100..1120: Configure current foreground window
const CMD_PICK_BASE: usize = 1200;   // 1200..1211: Pick .exe via child-process file dialog
const CMD_ACTIVE_BASE: usize = 2000; // 2000 + i * STRIDE + slot: Configure active process `i`
const CMD_RULE_BASE: usize = 3000;   // 3000 + i * STRIDE + slot: Modify/Delete existing rule `i`

struct AppState {
    hwnd: HWND,
    config: AppConfig,
    config_path: PathBuf,
    config_mtime: Option<SystemTime>,
    is_msix: bool,
    is_admin: bool,
    topo_summary: String,
    presets: Vec<CpuPreset>,
    engine: Engine,
    icon_normal: HICON,
    icon_restraining: HICON,
    icon_paused: HICON,
    last_icon_state: u8,
    last_tip_tick: u32,
    menu_active_names: Vec<String>,
    menu_rule_patterns: Vec<String>,
}

impl AppState {
    pub fn is_chinese(&self) -> bool {
        Language::parse(&self.config.language).is_chinese()
    }
}

static mut APP: *mut AppState = std::ptr::null_mut();

fn main() {
    unsafe {
        let args: Vec<String> = std::env::args().collect();

        // Subprocess modal file pickers: keeps heavy Windows Shell / comdlg32 DLLs out of the resident daemon
        if let Some(pos) = args.iter().position(|a| a == "--pick-exe") {
            let title = args
                .get(pos + 1)
                .map(|s| s.as_str())
                .unwrap_or("选择要配置规则的程序 (.exe)");
            if let Some(exe_name) = pick_exe_file_name(NULL, title) {
                println!("{}", exe_name);
            }
            return;
        }

        if args.iter().any(|a| a == "--pick-import") {
            if let Some(path) = pick_file(
                NULL,
                "选择导入规则配置文件 (.json / .ini)",
                &[("规则配置文件 (*.json;*.ini)", "*.json;*.ini")],
            ) {
                println!("{}", path);
            }
            return;
        }

        if let Some(pos) = args.iter().position(|a| a == "--import") {
            if let Some(import_path_str) = args.get(pos + 1) {
                let (mut cfg, cfg_path, _) = AppConfig::load_or_create();
                if let Ok((aff_n, gpu_n)) =
                    cfg.import_process_lasso_file(Path::new(import_path_str))
                {
                    let _ = cfg.save(&cfg_path);
                    let mut eng = Engine::new();
                    eng.tick(&cfg);
                    println!(
                        "IMPORT_OK: rules={} gpu_rules={} applied_now={} config={}",
                        aff_n,
                        gpu_n,
                        eng.total_affinity_actions,
                        cfg_path.display()
                    );
                    return;
                }
            }
        }

        if args.iter().any(|a| a == "--detect-cpu") {
            let (summary, presets) = detect_cpu_presets();
            println!("CPU Topology: {}", summary);
            for (i, p) in presets.iter().enumerate() {
                println!("  Preset {}: {} -> spec={}", i + 1, p.label, p.spec);
            }
            return;
        }

        let mutex_name = to_wide("Local\\SmartScheduler_SingleInstance_Mutex");
        let h_mutex = CreateMutexW(NULL, 1, mutex_name.as_ptr());
        let allow_replace = args.iter().any(|a| a == "--elevated");
        if !allow_replace && h_mutex != NULL && GetLastError() == ERROR_ALREADY_EXISTS {
            return;
        }

        let is_admin = is_elevated();
        if is_admin {
            enable_debug_privilege();
        }

        let (config, config_path, is_msix) = AppConfig::load_or_create();

        if config.auto_elevate && !is_admin && !args.iter().any(|a| a == "--no-elevate") {
            if relaunch_as_admin() {
                return;
            }
        }

        let (topo_summary, mut presets) = detect_cpu_presets();
        presets.truncate(MAX_PRESETS);

        let h_instance = GetModuleHandleW(std::ptr::null());
        let class_name = to_wide("SmartSchedulerTrayHiddenClass");

        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as UINT,
            style: 0,
            lpfnWndProc: Some(wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: h_instance,
            hIcon: NULL,
            hCursor: NULL,
            hbrBackground: NULL,
            lpszMenuName: std::ptr::null(),
            lpszClassName: class_name.as_ptr(),
            hIconSm: NULL,
        };
        RegisterClassExW(&wc);

        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            to_wide("Smart Scheduler").as_ptr(),
            0,
            0,
            0,
            0,
            0,
            NULL,
            NULL,
            h_instance,
            NULL,
        );
        if hwnd == NULL {
            return;
        }

        let icon_normal = create_status_icon(0x10B981);      // Emerald green
        let icon_restraining = create_status_icon(0xF59E0B); // Amber orange (FairShare active)
        let icon_paused = create_status_icon(0x6B7280);      // Slate gray

        let config_mtime = file_mtime(&config_path);

        let mut app_box = Box::new(AppState {
            hwnd,
            config,
            config_path,
            config_mtime,
            is_msix,
            is_admin,
            topo_summary,
            presets,
            engine: Engine::new(),
            icon_normal,
            icon_restraining,
            icon_paused,
            last_icon_state: 255,
            last_tip_tick: 0,
            menu_active_names: Vec::new(),
            menu_rule_patterns: Vec::new(),
        });

        app_box.engine.tick(&app_box.config);
        APP = Box::into_raw(app_box);

        update_tray_icon(true, true);
        SetTimer(hwnd, TIMER_ID_TICK, (*APP).config.interval_ms, NULL);
        optimize_heap_and_trim_working_set();

        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, NULL, 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        remove_tray_icon(hwnd);
        if !APP.is_null() {
            let mut app = Box::from_raw(APP);
            APP = std::ptr::null_mut();
            app.engine.restore_all_restrained();
            DestroyIcon(app.icon_normal);
            DestroyIcon(app.icon_restraining);
            DestroyIcon(app.icon_paused);
        }
    }
}

fn file_mtime(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).and_then(|m| m.modified()).ok()
}

fn save_config_and_record_mtime(app: &mut AppState) {
    let _ = app.config.save(&app.config_path);
    app.config_mtime = file_mtime(&app.config_path);
}

/// Runs a modal file dialog in a short-lived child instance of `smart-scheduler.exe`
/// so `comdlg32.dll` and Explorer Shell Extensions never bloat the resident daemon's RAM.
fn run_child_picker(args: &[&str]) -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    let output = std::process::Command::new(exe).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!s.is_empty()).then_some(s)
}

unsafe fn relaunch_as_admin() -> bool {
    let mut local_app_data = vec![0u16; 512];
    let env_name = to_wide("LOCALAPPDATA");
    let env_len = GetEnvironmentVariableW(env_name.as_ptr(), local_app_data.as_mut_ptr(), 512);
    let target_exe_wide = if env_len > 0 {
        let base = String::from_utf16_lossy(&local_app_data[..env_len as usize]);
        let alias_path = PathBuf::from(base)
            .join("Microsoft")
            .join("WindowsApps")
            .join("smart-scheduler.exe");
        if alias_path.exists() {
            to_wide(&alias_path.to_string_lossy())
        } else {
            let mut exe_buf = vec![0u16; 1024];
            let n = GetModuleFileNameW(NULL as _, exe_buf.as_mut_ptr(), 1024);
            if n == 0 {
                return false;
            }
            exe_buf.truncate(n as usize + 1);
            exe_buf
        }
    } else {
        let mut exe_buf = vec![0u16; 1024];
        let n = GetModuleFileNameW(NULL as _, exe_buf.as_mut_ptr(), 1024);
        if n == 0 {
            return false;
        }
        exe_buf.truncate(n as usize + 1);
        exe_buf
    };

    let verb = to_wide("runas");
    let params = to_wide("--elevated");
    let res = ShellExecuteW(
        NULL,
        verb.as_ptr(),
        target_exe_wide.as_ptr(),
        params.as_ptr(),
        std::ptr::null(),
        SW_SHOWNORMAL,
    ) as isize;
    res > 32
}

unsafe fn update_tray_icon(is_add: bool, update_tip: bool) {
    if APP.is_null() {
        return;
    }
    let app = &mut *APP;

    let desired_state = if !app.config.affinity_enabled && !app.config.fairshare.enabled {
        2u8
    } else if app.engine.active_restrained_count > 0 {
        1u8
    } else {
        0u8
    };

    if !is_add && !update_tip && desired_state == app.last_icon_state {
        return;
    }

    let icon = match desired_state {
        1 => app.icon_restraining,
        2 => app.icon_paused,
        _ => app.icon_normal,
    };

    let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
    nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as DWORD;
    nid.hWnd = app.hwnd;
    nid.uID = 1;
    nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    nid.uCallbackMessage = WM_TRAYICON;
    nid.hIcon = icon;

    let is_zh = app.is_chinese();
    let tip_str = if is_zh {
        format!(
            "{}\nCPU: {:.1}% | 规则生效: {} 次 | 公平调度: {} 次 ({})",
            app.topo_summary,
            app.engine.last_total_cpu_pct,
            app.engine.total_affinity_actions,
            app.engine.total_fairshare_actions,
            if app.is_admin { "管理员" } else { "普通权限" },
        )
    } else {
        format!(
            "{}\nCPU: {:.1}% | Rules Applied: {} | FairShare: {} ({})",
            app.topo_summary,
            app.engine.last_total_cpu_pct,
            app.engine.total_affinity_actions,
            app.engine.total_fairshare_actions,
            if app.is_admin { "Administrator" } else { "Standard" },
        )
    };
    let tip_wide = to_wide(&tip_str);
    let copy_len = tip_wide.len().min(127);
    nid.szTip[..copy_len].copy_from_slice(&tip_wide[..copy_len]);
    nid.szTip[copy_len] = 0;

    Shell_NotifyIconW(if is_add { NIM_ADD } else { NIM_MODIFY }, &nid);
    app.last_icon_state = desired_state;
    app.last_tip_tick = app.engine.tick_counter;
}

unsafe fn remove_tray_icon(hwnd: HWND) {
    let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
    nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as DWORD;
    nid.hWnd = hwnd;
    nid.uID = 1;
    Shell_NotifyIconW(NIM_DELETE, &nid);
}

/// Formats a rule summary, replacing raw core ranges with friendly CPU preset labels when matched.
fn friendly_rule_summary(rule: &Rule, presets: &[CpuPreset], is_zh: bool) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(spec) = &rule.affinity_str {
        if let Some(p) = presets.iter().find(|p| &p.spec == spec) {
            parts.push(p.display_label(is_zh).to_string());
        } else {
            parts.push(if is_zh {
                format!("核心 {}", spec)
            } else {
                format!("Cores {}", spec)
            });
        }
    }
    if let Some(prio) = rule.priority {
        let p_name = match prio {
            HIGH_PRIORITY_CLASS => if is_zh { "高优先级" } else { "High Priority" },
            ABOVE_NORMAL_PRIORITY_CLASS => if is_zh { "高于正常" } else { "Above Normal" },
            NORMAL_PRIORITY_CLASS => if is_zh { "正常优先级" } else { "Normal Priority" },
            BELOW_NORMAL_PRIORITY_CLASS => if is_zh { "低于正常" } else { "Below Normal" },
            IDLE_PRIORITY_CLASS => if is_zh { "空闲优先级" } else { "Idle Priority" },
            _ => if is_zh { "自定义优先级" } else { "Custom Priority" },
        };
        parts.push(p_name.into());
    }
    match rule.fairshare {
        FairShareMode::Exempt => parts.push(if is_zh { "FS豁免".into() } else { "FS Exempt".into() }),
        FairShareMode::Force => parts.push(if is_zh { "FS强力".into() } else { "FS Force".into() }),
        FairShareMode::Default => {}
    }
    if rule.delay_ms > 0 {
        parts.push(if is_zh {
            format!("延迟{}ms", rule.delay_ms)
        } else {
            format!("Delay {}ms", rule.delay_ms)
        });
    }
    parts.join(" · ")
}

unsafe fn menu_item(menu: HMENU, id: usize, label: &str) {
    AppendMenuW(menu, MF_STRING, id, to_wide(label).as_ptr());
}

unsafe fn menu_check(menu: HMENU, id: usize, checked: bool, label: &str) {
    let flag = if checked { MF_CHECKED } else { MF_UNCHECKED };
    AppendMenuW(menu, MF_STRING | flag, id, to_wide(label).as_ptr());
}

unsafe fn menu_popup(menu: HMENU, sub: HMENU, label: &str) {
    AppendMenuW(menu, MF_POPUP, sub as usize, to_wide(label).as_ptr());
}

unsafe fn menu_sep(menu: HMENU) {
    AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
}

/// Populates a per-process configuration submenu (CPU Affinity + Priority + FairShare + Persist + Remove)
unsafe fn populate_proc_submenu(
    sub: HMENU,
    presets: &[CpuPreset],
    cur_rule: Option<&Rule>,
    base_cmd: usize,
    is_zh: bool,
) {
    let cur_spec = cur_rule.and_then(|r| r.affinity_str.as_deref());
    let cur_prio = cur_rule.and_then(|r| r.priority);
    let cur_fs = cur_rule.map_or(FairShareMode::Default, |r| r.fairshare);
    let cur_persist = cur_rule.map_or(true, |r| r.persist);

    for (p_idx, preset) in presets.iter().enumerate() {
        let checked = cur_spec == Some(preset.spec.as_str());
        let label = if is_zh {
            format!("绑核: {}", preset.display_label(true))
        } else {
            format!("Affinity: {}", preset.display_label(false))
        };
        menu_check(sub, base_cmd + p_idx, checked, &label);
    }
    menu_check(
        sub,
        base_cmd + SLOT_AFFINITY_ALL,
        cur_spec.is_none(),
        if is_zh { "绑核: 不限制 (全核)" } else { "Affinity: Default (All Cores)" },
    );

    menu_sep(sub);
    let prio_items: &[(usize, Option<DWORD>, &str, &str)] = &[
        (SLOT_PRIO_NONE, None, "优先级: 默认 (不干预)", "Priority: Default (Unchanged)"),
        (SLOT_PRIO_HIGH, Some(HIGH_PRIORITY_CLASS), "优先级: 高 (High)", "Priority: High"),
        (SLOT_PRIO_ABOVE, Some(ABOVE_NORMAL_PRIORITY_CLASS), "优先级: 高于正常 (Above Normal)", "Priority: Above Normal"),
        (SLOT_PRIO_NORMAL, Some(NORMAL_PRIORITY_CLASS), "优先级: 正常 (Normal)", "Priority: Normal"),
        (SLOT_PRIO_BELOW, Some(BELOW_NORMAL_PRIORITY_CLASS), "优先级: 低于正常 (Below Normal)", "Priority: Below Normal"),
        (SLOT_PRIO_IDLE, Some(IDLE_PRIORITY_CLASS), "优先级: 空闲 (Idle)", "Priority: Idle"),
    ];
    for &(slot, prio_val, label_zh, label_en) in prio_items {
        menu_check(sub, base_cmd + slot, cur_prio == prio_val, if is_zh { label_zh } else { label_en });
    }

    menu_sep(sub);
    let fs_items: &[(usize, FairShareMode, &str, &str)] = &[
        (SLOT_FS_DEFAULT, FairShareMode::Default, "FairShare: 默认跟随全局", "FairShare: Default (Follow Global)"),
        (SLOT_FS_EXEMPT, FairShareMode::Exempt, "FairShare: 豁免 (绝不降级)", "FairShare: Exempt (Never Restrain)"),
        (SLOT_FS_FORCE, FairShareMode::Force, "FairShare: 强力调度 (超阈值即降级)", "FairShare: Force (Always Restrain)"),
    ];
    for &(slot, mode, label_zh, label_en) in fs_items {
        menu_check(sub, base_cmd + slot, cur_fs == mode, if is_zh { label_zh } else { label_en });
    }
    menu_check(
        sub,
        base_cmd + SLOT_TOGGLE_PERSIST,
        cur_persist,
        if is_zh { "定期复查防重置 (Persist)" } else { "Persistent Enforce (Re-apply affinity)" },
    );

    if cur_rule.is_some() {
        menu_sep(sub);
        menu_item(
            sub,
            base_cmd + SLOT_REMOVE_RULE,
            if is_zh { "移除此规则并恢复默认" } else { "Remove Rule (Restore Defaults)" },
        );
    }
}

unsafe fn show_tray_menu(hwnd: HWND) {
    if APP.is_null() {
        return;
    }
    let app = &mut *APP;
    let is_zh = app.is_chinese();
    let menu = CreatePopupMenu();
    if menu == NULL {
        return;
    }

    let header = to_wide(&if is_zh {
        format!(
            "{}  CPU: {:.1}% | 生效: {} | 调度: {}",
            app.topo_summary,
            app.engine.last_total_cpu_pct,
            app.engine.total_affinity_actions,
            app.engine.total_fairshare_actions
        )
    } else {
        format!(
            "{}  CPU: {:.1}% | Applied: {} | FairShare: {}",
            app.topo_summary,
            app.engine.last_total_cpu_pct,
            app.engine.total_affinity_actions,
            app.engine.total_fairshare_actions
        )
    });
    AppendMenuW(menu, MF_STRING | MF_GRAYED, 0, header.as_ptr());
    menu_sep(menu);

    if !app.engine.last_user_fg_name.is_empty() {
        let fg_sub = CreatePopupMenu();
        let fg_name = &app.engine.last_user_fg_name;
        let cur_rule = app.config.find_rule_by_name(fg_name);
        populate_proc_submenu(fg_sub, &app.presets, cur_rule, CMD_FG_BASE, is_zh);
        let label = match cur_rule {
            Some(r) => {
                if is_zh {
                    format!("配置当前窗口: {} [{}]", fg_name, r.summary_localized(true))
                } else {
                    format!("Foreground Window: {} [{}]", fg_name, r.summary_localized(false))
                }
            }
            None => {
                if is_zh {
                    format!("配置当前窗口: {}", fg_name)
                } else {
                    format!("Foreground Window: {}", fg_name)
                }
            }
        };
        menu_popup(menu, fg_sub, &label);
    }

    let active_menu = CreatePopupMenu();
    let top_active = app.engine.get_top_active_apps(&app.config, TOP_ACTIVE_LIMIT);
    app.menu_active_names.clear();
    for (idx, (proc_name, cnt, cpu_pct, rule_opt)) in top_active.into_iter().enumerate() {
        let sub = CreatePopupMenu();
        populate_proc_submenu(sub, &app.presets, rule_opt.as_ref(), CMD_ACTIVE_BASE + idx * STRIDE, is_zh);
        let item_title = match &rule_opt {
            Some(r) => {
                if is_zh {
                    format!("✓ {} ({:.1}%, {}个) [{}]", proc_name, cpu_pct, cnt, r.summary_localized(true))
                } else {
                    format!("✓ {} ({:.1}%, {}x) [{}]", proc_name, cpu_pct, cnt, r.summary_localized(false))
                }
            }
            None => {
                if is_zh {
                    format!("   {} ({:.1}%, {}个)", proc_name, cpu_pct, cnt)
                } else {
                    format!("   {} ({:.1}%, {}x)", proc_name, cpu_pct, cnt)
                }
            }
        };
        menu_popup(active_menu, sub, &item_title);
        app.menu_active_names.push(proc_name);
    }
    menu_popup(
        menu,
        active_menu,
        if is_zh { "选择活跃进程配置 (按 CPU 排序 Top 12)" } else { "Active Processes (Top 12 by CPU)" },
    );

    let pick_menu = CreatePopupMenu();
    for (p_idx, preset) in app.presets.iter().enumerate() {
        let text = if is_zh {
            format!("选择 .exe 绑核到 -> {}", preset.display_label(true))
        } else {
            format!("Assign .exe Affinity -> {}", preset.display_label(false))
        };
        menu_item(pick_menu, CMD_PICK_BASE + p_idx, &text);
    }
    menu_sep(pick_menu);
    menu_item(
        pick_menu,
        CMD_PICK_BASE + PICK_EXTRA_HIGH_EXEMPT,
        if is_zh { "选择 .exe 设为 -> 高优先级 + FairShare 豁免" } else { "Set .exe -> High Priority + FairShare Exempt" },
    );
    menu_item(
        pick_menu,
        CMD_PICK_BASE + PICK_EXTRA_EXEMPT_ONLY,
        if is_zh { "选择 .exe 设为 -> FairShare 豁免 (不限核)" } else { "Set .exe -> FairShare Exempt (All Cores)" },
    );
    menu_item(
        pick_menu,
        CMD_PICK_BASE + PICK_EXTRA_FORCE_LOW,
        if is_zh { "选择 .exe 设为 -> 低于正常优先级 + 强力调度" } else { "Set .exe -> Below Normal Priority + Force Restraint" },
    );
    menu_popup(
        menu,
        pick_menu,
        if is_zh { "浏览选择 .exe 文件添加规则..." } else { "Browse .exe to Add Rule..." },
    );

    let rules_menu = CreatePopupMenu();
    app.menu_rule_patterns.clear();
    if app.config.rules.is_empty() {
        AppendMenuW(
            rules_menu,
            MF_STRING | MF_GRAYED,
            0,
            to_wide(if is_zh {
                "暂无进程规则 (支持在 config.toml 中使用 * 通配符或路径)"
            } else {
                "No rules configured (wildcards & paths supported in config.toml)"
            }).as_ptr(),
        );
    } else {
        for (idx, r) in app.config.rules.iter().enumerate() {
            let sub = CreatePopupMenu();
            populate_proc_submenu(sub, &app.presets, Some(r), CMD_RULE_BASE + idx * STRIDE, is_zh);
            let label = format!("{}  ->  {}", r.pattern, friendly_rule_summary(r, &app.presets, is_zh));
            menu_popup(rules_menu, sub, &label);
            app.menu_rule_patterns.push(r.pattern.clone());
        }
    }
    menu_popup(
        menu,
        rules_menu,
        &if is_zh {
            format!("已配置规则管理 ({} 条)", app.config.rules.len())
        } else {
            format!("Configured Rules ({} active)", app.config.rules.len())
        },
    );

    menu_sep(menu);
    menu_check(
        menu,
        CMD_TOGGLE_AFFINITY,
        app.config.affinity_enabled,
        if is_zh { "启用进程规则自动执行 (绑核 / 优先级)" } else { "Enable Rule Enforcement (Affinity / Priority)" },
    );
    menu_check(
        menu,
        CMD_TOGGLE_FAIRSHARE,
        app.config.fairshare.enabled,
        if is_zh { "启用 FairShare 公平调度 (动态平衡)" } else { "Enable FairShare Dynamic Scheduling" },
    );

    let settings_menu = CreatePopupMenu();
    let events_menu = CreatePopupMenu();
    for ev in app.engine.recent_events.iter().rev() {
        AppendMenuW(events_menu, MF_STRING | MF_GRAYED, 0, to_wide(ev).as_ptr());
    }
    menu_popup(
        settings_menu,
        events_menu,
        if is_zh { "最近调度日志 (实时)" } else { "Recent Scheduling Logs (Live)" },
    );
    menu_sep(settings_menu);

    let lang_menu = CreatePopupMenu();
    let cur_lang = Language::parse(&app.config.language);
    menu_check(
        lang_menu,
        CMD_LANG_AUTO,
        cur_lang == Language::Auto,
        if is_zh { "跟随系统 / Auto (System Default)" } else { "Auto (System Default)" },
    );
    menu_check(lang_menu, CMD_LANG_ZH, cur_lang == Language::ZhCn, "简体中文 (Chinese)");
    menu_check(lang_menu, CMD_LANG_EN, cur_lang == Language::EnUs, "English");
    menu_popup(
        settings_menu,
        lang_menu,
        if is_zh { "界面语言 / Language" } else { "Language / 语言" },
    );

    if !app.is_admin {
        menu_item(
            settings_menu,
            CMD_ELEVATE_NOW,
            if is_zh { "立即以管理员权限重启" } else { "Restart as Administrator Now" },
        );
    }
    menu_check(
        settings_menu,
        CMD_TOGGLE_AUTO_ELEVATE,
        app.config.auto_elevate,
        if is_zh { "启动时自动请求管理员权限 (UAC)" } else { "Auto-Elevate on Startup (UAC)" },
    );
    menu_check(
        settings_menu,
        CMD_TOGGLE_AUTOSTART,
        autostart_enabled(),
        if is_zh {
            if app.is_msix { "开机自动启动 (注册表 Run 附加项)" } else { "开机自动启动 (当前用户 Run)" }
        } else {
            if app.is_msix { "Start with Windows (Registry Run)" } else { "Start with Windows (Startup Run)" }
        },
    );
    if app.is_msix {
        menu_item(
            settings_menu,
            CMD_STARTUP_SETTINGS,
            if is_zh { "Windows 启动应用设置 (MSIX StartupTask)..." } else { "Windows Startup Apps Settings (MSIX)..." },
        );
    }

    menu_sep(settings_menu);
    menu_item(
        settings_menu,
        CMD_OPEN_CONFIG,
        if is_zh { "编辑配置文件 (config.toml，保存自动生效)" } else { "Edit Configuration (config.toml)" },
    );
    menu_item(
        settings_menu,
        CMD_RELOAD_CONFIG,
        if is_zh { "立即重新加载配置" } else { "Reload Configuration Now" },
    );
    menu_item(
        settings_menu,
        CMD_IMPORT_PL_FILE,
        if is_zh { "导入规则配置 (.json / .ini)..." } else { "Import Rules (.json / .ini)..." },
    );
    menu_popup(
        menu,
        settings_menu,
        if is_zh { "设置与日志" } else { "Settings & Logs" },
    );

    menu_sep(menu);
    menu_item(menu, CMD_EXIT, if is_zh { "退出" } else { "Exit" });

    let mut pt = POINT { x: 0, y: 0 };
    GetCursorPos(&mut pt);
    SetForegroundWindow(hwnd);
    TrackPopupMenu(
        menu,
        TPM_LEFTALIGN | TPM_BOTTOMALIGN | TPM_RIGHTBUTTON,
        pt.x,
        pt.y,
        0,
        hwnd,
        std::ptr::null(),
    );
    PostMessageW(hwnd, 0, 0, 0);
    DestroyMenu(menu);
    optimize_heap_and_trim_working_set();
}

unsafe fn apply_proc_slot_action(app: &mut AppState, pattern: &str, slot: usize) {
    let clean_pat = pattern.trim().to_ascii_lowercase();
    if clean_pat.is_empty() {
        return;
    }

    let existing_pos = app
        .config
        .rules
        .iter()
        .position(|r| r.pattern == clean_pat || r.is_exact_for(&clean_pat));

    if slot == SLOT_REMOVE_RULE {
        if let Some(pos) = existing_pos {
            app.config.rules.remove(pos);
        }
        let restored = app.engine.reset_process_by_pattern(&clean_pat, true, true);
        app.engine.push_event(format!(
            "[{}] 移除规则并恢复默认: {} ({} 个实例)",
            now_hms(),
            clean_pat,
            restored
        ));
        save_config_and_record_mtime(app);
        app.engine.invalidate_rule_cache(&app.config);
        app.engine.tick(&app.config);
        update_tray_icon(false, false);
        return;
    }

    let mut rule = match existing_pos {
        Some(pos) => app.config.rules[pos].clone(),
        None => {
            let mut base = Rule::base(&clean_pat).unwrap_or_else(|| unreachable!());
            if !base.is_glob && !base.match_path {
                if let Some(inherited) = app.config.find_rule_by_name(&clean_pat) {
                    base.affinity_str = inherited.affinity_str.clone();
                    base.affinity_mask = inherited.affinity_mask;
                    base.priority = inherited.priority;
                    base.fairshare = inherited.fairshare;
                    base.delay_ms = inherited.delay_ms;
                    base.persist = inherited.persist;
                }
            }
            base
        }
    };

    let mut already_reset_aff = false;
    let mut already_reset_prio = false;

    if slot < MAX_PRESETS {
        if let Some(preset) = app.presets.get(slot) {
            rule.set_affinity(&preset.spec);
        }
    } else {
        match slot {
            SLOT_AFFINITY_ALL => {
                rule.affinity_str = None;
                rule.affinity_mask = None;
                app.engine.reset_process_by_pattern(&clean_pat, true, false);
                already_reset_aff = true;
            }
            SLOT_PRIO_NONE => {
                rule.priority = None;
                app.engine.reset_process_by_pattern(&clean_pat, false, true);
                already_reset_prio = true;
            }
            SLOT_PRIO_HIGH => rule.priority = Some(HIGH_PRIORITY_CLASS),
            SLOT_PRIO_ABOVE => rule.priority = Some(ABOVE_NORMAL_PRIORITY_CLASS),
            SLOT_PRIO_NORMAL => rule.priority = Some(NORMAL_PRIORITY_CLASS),
            SLOT_PRIO_BELOW => rule.priority = Some(BELOW_NORMAL_PRIORITY_CLASS),
            SLOT_PRIO_IDLE => rule.priority = Some(IDLE_PRIORITY_CLASS),
            SLOT_FS_DEFAULT => rule.fairshare = FairShareMode::Default,
            SLOT_FS_EXEMPT => rule.fairshare = FairShareMode::Exempt,
            SLOT_FS_FORCE => rule.fairshare = FairShareMode::Force,
            SLOT_TOGGLE_PERSIST => rule.persist = !rule.persist,
            _ => return,
        }
    }

    if !rule.has_settings() {
        if let Some(pos) = existing_pos {
            app.config.rules.remove(pos);
        }
        let restored = app.engine.reset_process_by_pattern(
            &clean_pat,
            !already_reset_aff,
            !already_reset_prio,
        );
        app.engine.push_event(format!(
            "[{}] 规则已清空并恢复默认: {} ({} 个实例)",
            now_hms(),
            clean_pat,
            restored
        ));
    } else {
        let summary = rule.summary();
        if let Some(pos) = existing_pos {
            app.config.rules[pos] = rule;
        } else {
            app.config.rules.push(rule);
        }
        app.engine.push_event(format!(
            "[{}] 设置规则: {} -> {}",
            now_hms(),
            clean_pat,
            summary
        ));
    }

    save_config_and_record_mtime(app);
    app.engine.invalidate_rule_cache(&app.config);
    app.engine.tick(&app.config);
    update_tray_icon(false, false);
}

unsafe fn reload_config_from_disk(app: &mut AppState, reason: &str) {
    let (new_cfg, _, _) = AppConfig::load_or_create();
    let interval_changed = new_cfg.interval_ms != app.config.interval_ms;
    app.config = new_cfg;
    app.config_mtime = file_mtime(&app.config_path);
    if interval_changed {
        KillTimer(app.hwnd, TIMER_ID_TICK);
        SetTimer(app.hwnd, TIMER_ID_TICK, app.config.interval_ms, NULL);
    }
    app.engine.invalidate_rule_cache(&app.config);
    app.engine.push_event(format!(
        "[{}] {} ({} 条规则)",
        now_hms(),
        reason,
        app.config.rules.len()
    ));
    app.engine.tick(&app.config);
    update_tray_icon(false, false);
}

unsafe fn import_pl_config_via_picker(app: &mut AppState) {
    let Some(picked_path) = run_child_picker(&["--pick-import"]) else {
        return;
    };
    let path = Path::new(&picked_path);
    if let Ok((aff_count, gpu_count)) = app.config.import_process_lasso_file(path) {
        save_config_and_record_mtime(app);
        app.engine.invalidate_rule_cache(&app.config);
        app.engine.push_event(format!(
            "[{}] 已导入 {}: {}条进程规则 + {}条GPU规则",
            now_hms(),
            path.file_name().unwrap_or_default().to_string_lossy(),
            aff_count,
            gpu_count
        ));
        app.engine.tick(&app.config);
        update_tray_icon(false, true);
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: UINT,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_TIMER => {
            if wparam == TIMER_ID_TICK && !APP.is_null() {
                let app = &mut *APP;
                // Hot-reload config.toml every 5 ticks when modified in an external editor
                if app.engine.tick_counter % 5 == 0 {
                    let cur_mtime = file_mtime(&app.config_path);
                    if cur_mtime.is_some() && cur_mtime != app.config_mtime {
                        reload_config_from_disk(app, "配置文件已自动热重载");
                        return 0;
                    }
                }
                app.engine.tick(&app.config);
                update_tray_icon(false, false);
            }
            0
        }
        WM_TRAYICON => {
            let event = (lparam as u32) & 0xFFFF;
            if event == WM_MOUSEMOVE && !APP.is_null() {
                let app = &mut *APP;
                if app.last_tip_tick != app.engine.tick_counter {
                    update_tray_icon(false, true);
                }
            } else if event == WM_RBUTTONUP || event == WM_CONTEXTMENU || event == WM_LBUTTONUP {
                update_tray_icon(false, true);
                show_tray_menu(hwnd);
            }
            0
        }
        WM_COMMAND => {
            if APP.is_null() {
                return 0;
            }
            let app = &mut *APP;
            let cmd = wparam & 0xFFFF;

            // 1. Current Foreground App Configuration (CMD_FG_BASE..CMD_FG_BASE + STRIDE)
            if cmd >= CMD_FG_BASE && cmd < CMD_FG_BASE + STRIDE {
                let slot = cmd - CMD_FG_BASE;
                let fg_name = app.engine.last_user_fg_name.clone();
                apply_proc_slot_action(app, &fg_name, slot);
                return 0;
            }

            // 2. Child-Process File Picker (.exe) Rule Creation (CMD_PICK_BASE..CMD_PICK_BASE + PICK_TOTAL_SLOTS)
            if cmd >= CMD_PICK_BASE && cmd < CMD_PICK_BASE + PICK_TOTAL_SLOTS {
                let slot = cmd - CMD_PICK_BASE;
                if slot < MAX_PRESETS {
                    if let Some(preset) = app.presets.get(slot).cloned() {
                        let title = format!("选择要绑核到【{}】的程序 (.exe)", preset.label);
                        if let Some(exe_name) = run_child_picker(&["--pick-exe", &title]) {
                            apply_proc_slot_action(app, &exe_name, slot);
                        }
                    }
                } else {
                    let title = "选择要配置规则的程序 (.exe)";
                    if let Some(exe_name) = run_child_picker(&["--pick-exe", title]) {
                        match slot {
                            PICK_EXTRA_HIGH_EXEMPT => {
                                apply_proc_slot_action(app, &exe_name, SLOT_PRIO_HIGH);
                                apply_proc_slot_action(app, &exe_name, SLOT_FS_EXEMPT);
                            }
                            PICK_EXTRA_EXEMPT_ONLY => {
                                apply_proc_slot_action(app, &exe_name, SLOT_FS_EXEMPT);
                            }
                            PICK_EXTRA_FORCE_LOW => {
                                apply_proc_slot_action(app, &exe_name, SLOT_PRIO_BELOW);
                                apply_proc_slot_action(app, &exe_name, SLOT_FS_FORCE);
                            }
                            _ => {}
                        }
                    }
                }
                return 0;
            }

            // 3. Top-12 Active Processes Submenu (CMD_ACTIVE_BASE..)
            if cmd >= CMD_ACTIVE_BASE && cmd < CMD_ACTIVE_BASE + TOP_ACTIVE_LIMIT * STRIDE {
                let rel = cmd - CMD_ACTIVE_BASE;
                let proc_idx = rel / STRIDE;
                let slot = rel % STRIDE;
                if let Some(proc_name) = app.menu_active_names.get(proc_idx).cloned() {
                    apply_proc_slot_action(app, &proc_name, slot);
                }
                return 0;
            }

            // 4. Existing Rules Management Submenu (CMD_RULE_BASE..)
            if cmd >= CMD_RULE_BASE && cmd < CMD_RULE_BASE + 512 * STRIDE {
                let rel = cmd - CMD_RULE_BASE;
                let rule_idx = rel / STRIDE;
                let slot = rel % STRIDE;
                if let Some(pattern) = app.menu_rule_patterns.get(rule_idx).cloned() {
                    apply_proc_slot_action(app, &pattern, slot);
                }
                return 0;
            }

            match cmd {
                CMD_TOGGLE_AFFINITY => {
                    app.config.affinity_enabled = !app.config.affinity_enabled;
                    save_config_and_record_mtime(app);
                    app.engine.invalidate_rule_cache(&app.config);
                    update_tray_icon(false, true);
                }
                CMD_TOGGLE_FAIRSHARE => {
                    app.config.fairshare.enabled = !app.config.fairshare.enabled;
                    if !app.config.fairshare.enabled {
                        app.engine.restore_all_restrained();
                    }
                    save_config_and_record_mtime(app);
                    update_tray_icon(false, true);
                }
                CMD_TOGGLE_AUTO_ELEVATE => {
                    app.config.auto_elevate = !app.config.auto_elevate;
                    save_config_and_record_mtime(app);
                }
                CMD_LANG_AUTO => {
                    app.config.language = "auto".to_string();
                    save_config_and_record_mtime(app);
                    update_tray_icon(false, true);
                }
                CMD_LANG_ZH => {
                    app.config.language = "zh-CN".to_string();
                    save_config_and_record_mtime(app);
                    update_tray_icon(false, true);
                }
                CMD_LANG_EN => {
                    app.config.language = "en-US".to_string();
                    save_config_and_record_mtime(app);
                    update_tray_icon(false, true);
                }
                CMD_TOGGLE_AUTOSTART => {
                    let target = !autostart_enabled();
                    if set_autostart(target) {
                        app.engine.push_event(format!(
                            "[{}] 开机自启已{}",
                            now_hms(),
                            if target { "开启" } else { "关闭" }
                        ));
                    }
                }
                CMD_ELEVATE_NOW => {
                    if relaunch_as_admin() {
                        PostQuitMessage(0);
                    }
                }
                CMD_OPEN_CONFIG => {
                    let verb = to_wide("open");
                    let file = to_wide("notepad.exe");
                    let param = to_wide(&format!("\"{}\"", app.config_path.display()));
                    ShellExecuteW(
                        hwnd,
                        verb.as_ptr(),
                        file.as_ptr(),
                        param.as_ptr(),
                        std::ptr::null(),
                        SW_SHOWNORMAL,
                    );
                }
                CMD_RELOAD_CONFIG => {
                    reload_config_from_disk(app, "已手动重新加载配置");
                }
                CMD_IMPORT_PL_FILE => {
                    import_pl_config_via_picker(app);
                }
                CMD_STARTUP_SETTINGS => {
                    let verb = to_wide("open");
                    let uri = to_wide("ms-settings:startupapps");
                    ShellExecuteW(
                        hwnd,
                        verb.as_ptr(),
                        uri.as_ptr(),
                        std::ptr::null(),
                        std::ptr::null(),
                        SW_SHOWNORMAL,
                    );
                }
                CMD_EXIT => {
                    PostQuitMessage(0);
                }
                _ => {}
            }
            0
        }
        WM_CLOSE | WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
