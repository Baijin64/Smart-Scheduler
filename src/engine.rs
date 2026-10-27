use std::collections::{HashMap, VecDeque};
use std::ffi::c_void;

use crate::config::{is_critical_system_process, AppConfig, FairShareMode, Rule};
use crate::win32::*;

const MAX_RECENT_EVENTS: usize = 18;
const NO_RULE: u16 = u16::MAX;
const FLAG_CRITICAL_IMMUNE: u8 = 1 << 0;
const FLAG_RESTRAINED: u8 = 1 << 1;

/// A space-efficient process name storing names up to 22 ASCII bytes inline (24 bytes total).
#[derive(Clone, Eq, PartialEq, Hash)]
pub enum CompactName {
    Inline([u8; 22], u8),
    Heap(Box<str>),
}

impl CompactName {
    pub const fn empty() -> Self {
        CompactName::Inline([0u8; 22], 0)
    }

    pub fn from_utf16_lower(slice: &[u16]) -> Self {
        if slice.len() <= 22 {
            let mut buf = [0u8; 22];
            let mut is_ascii = true;
            for (i, &c) in slice.iter().enumerate() {
                if c < 128 {
                    buf[i] = (c as u8).to_ascii_lowercase();
                } else {
                    is_ascii = false;
                    break;
                }
            }
            if is_ascii {
                return CompactName::Inline(buf, slice.len() as u8);
            }
        }
        let mut s = String::from_utf16_lossy(slice);
        s.make_ascii_lowercase();
        CompactName::Heap(s.into_boxed_str())
    }

    pub fn from_str(s: &str) -> Self {
        let bytes = s.as_bytes();
        if bytes.len() <= 22 {
            let mut buf = [0u8; 22];
            for (i, &b) in bytes.iter().enumerate() {
                buf[i] = b.to_ascii_lowercase();
            }
            CompactName::Inline(buf, bytes.len() as u8)
        } else {
            let lower = s.to_ascii_lowercase();
            CompactName::Heap(lower.into_boxed_str())
        }
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        match self {
            CompactName::Inline(buf, len) => unsafe {
                std::str::from_utf8_unchecked(&buf[..*len as usize])
            },
            CompactName::Heap(b) => b.as_ref(),
        }
    }
}

impl PartialEq<str> for CompactName {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<CompactName> for str {
    #[inline]
    fn eq(&self, other: &CompactName) -> bool {
        self == other.as_str()
    }
}

impl PartialEq<String> for CompactName {
    #[inline]
    fn eq(&self, other: &String) -> bool {
        self.as_str() == other.as_str()
    }
}

impl PartialEq<CompactName> for String {
    #[inline]
    fn eq(&self, other: &CompactName) -> bool {
        self.as_str() == other.as_str()
    }
}

impl std::ops::Deref for CompactName {
    type Target = str;
    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl std::borrow::Borrow<str> for CompactName {
    #[inline]
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl AsRef<str> for CompactName {
    #[inline]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::fmt::Display for CompactName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::fmt::Debug for CompactName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.as_str())
    }
}

#[derive(Copy, Clone)]
pub struct SnapItem {
    pub pid: u32,
    pub parent_pid: u32,
    pub session_id: u32,
    pub base_priority: i32,
    pub delta_time: i64,
}

/// Ultra-compact 64-byte per-process state (fits in a single 64B CPU cache line).
#[repr(C)]
#[derive(Clone)]
pub struct ProcState {
    pub create_time: i64,          // 8B
    pub last_total_time: i64,      // 8B (KernelTime + UserTime in 100ns units)
    pub name_lower: CompactName,   // 24B
    pub first_seen_rel_ms: u32,    // 4B (ms since Engine::start_ms)
    pub restrained_rel_ms: u32,    // 4B (ms since Engine::start_ms)
    pub seen_tick: u32,            // 4B
    pub last_cpu_pct: f32,         // 4B
    pub over_quota_ms: u16,        // 2B (up to 65.5s continuous over-quota)
    pub rule_idx: u16,             // 2B (`NO_RULE` = u16::MAX)
    pub original_priority: u16,    // 2B (all Win32 priority classes 0x20..0x8000 fit in u16)
    pub rule_applied: u8,          // 1B
    pub flags: u8,                 // 1B (FLAG_CRITICAL_IMMUNE | FLAG_RESTRAINED)
}

impl ProcState {
    #[inline]
    pub fn is_critical_immune(&self) -> bool {
        (self.flags & FLAG_CRITICAL_IMMUNE) != 0
    }

    #[inline]
    pub fn is_restrained(&self) -> bool {
        (self.flags & FLAG_RESTRAINED) != 0
    }

    #[inline]
    pub fn set_restrained(&mut self, val: bool) {
        if val {
            self.flags |= FLAG_RESTRAINED;
        } else {
            self.flags &= !FLAG_RESTRAINED;
        }
    }
}

pub struct Engine {
    pub snapshot_buf: Vec<u8>,
    pub items_buf: Vec<SnapItem>,
    pub procs: HashMap<u32, ProcState>,
    /// Side-table for full image paths; empty (0 capacity) unless a path-based rule is configured.
    pub proc_paths: HashMap<u32, Box<str>>,
    pub num_cpus: usize,
    pub self_pid: u32,
    pub start_ms: u64,
    pub last_tick_ms: u64,
    pub tick_counter: u32,
    pub total_affinity_actions: u64,
    pub total_fairshare_actions: u64,
    pub active_restrained_count: usize,
    pub last_total_cpu_pct: f32,
    pub recent_events: VecDeque<String>,
    pub last_fg_hwnd: HWND,
    pub last_fg_pid: u32,
    pub last_user_fg_pid: u32,
    pub last_user_fg_name: String,
}

impl Engine {
    pub fn new() -> Self {
        let num_cpus = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(32)
            .max(1);
        let self_pid = std::process::id();
        let now = unsafe { GetTickCount64() };

        let mut events = VecDeque::with_capacity(MAX_RECENT_EVENTS);
        events.push_back(format!(
            "[{}] 引擎已启动 (逻辑核心: {})",
            now_hms(),
            num_cpus
        ));

        Self {
            snapshot_buf: vec![0u8; 128 * 1024],
            items_buf: Vec::with_capacity(64),
            procs: HashMap::with_capacity(384),
            proc_paths: HashMap::new(),
            num_cpus,
            self_pid,
            start_ms: now,
            last_tick_ms: now,
            tick_counter: 0,
            total_affinity_actions: 0,
            total_fairshare_actions: 0,
            active_restrained_count: 0,
            last_total_cpu_pct: 0.0,
            recent_events: events,
            last_fg_hwnd: NULL,
            last_fg_pid: 0,
            last_user_fg_pid: 0,
            last_user_fg_name: String::new(),
        }
    }

    #[inline]
    fn rel_ms(&self, now_ms: u64) -> u32 {
        now_ms.saturating_sub(self.start_ms).min(u32::MAX as u64) as u32
    }

    pub fn push_event(&mut self, msg: String) {
        if self.recent_events.len() >= MAX_RECENT_EVENTS {
            self.recent_events.pop_front();
        }
        self.recent_events.push_back(msg);
    }

    pub fn invalidate_rule_cache(&mut self, cfg: &AppConfig) {
        let has_path_rule = cfg.rules.iter().any(|r| r.match_path);
        if !has_path_rule && self.proc_paths.capacity() > 0 {
            self.proc_paths = HashMap::new();
        }
        for (&pid, state) in self.procs.iter_mut() {
            state.rule_applied = 0;
            state.rule_idx = lookup_rule(
                cfg,
                pid,
                &state.name_lower,
                state.is_critical_immune(),
                &mut self.proc_paths,
            );
        }
    }

    pub fn reset_process_by_pattern(
        &mut self,
        pattern: &str,
        reset_affinity: bool,
        reset_priority: bool,
    ) -> usize {
        let Some(matcher) = Rule::base(pattern) else {
            return 0;
        };
        let mut count = 0usize;
        for (&pid, st) in self.procs.iter_mut() {
            let path_opt: Option<&str> = if matcher.match_path {
                Some(
                    &**self.proc_paths.entry(pid).or_insert_with(|| {
                        process_image_path_lower(pid)
                            .unwrap_or_default()
                            .into_boxed_str()
                    }),
                )
            } else {
                self.proc_paths.get(&pid).map(|b| &**b)
            };
            if matcher.matches(&st.name_lower, path_opt) {
                st.rule_applied = 0;
                unsafe {
                    let h = OpenProcess(
                        PROCESS_SET_INFORMATION | PROCESS_QUERY_LIMITED_INFORMATION,
                        0,
                        pid,
                    );
                    if h != NULL && h != INVALID_HANDLE_VALUE {
                        let mut ok = false;
                        if reset_affinity {
                            let mut cur_mask: usize = 0;
                            let mut sys_mask: usize = 0;
                            if GetProcessAffinityMask(h, &mut cur_mask, &mut sys_mask) != 0
                                && sys_mask != 0
                                && SetProcessAffinityMask(h, sys_mask) != 0
                            {
                                ok = true;
                            }
                        }
                        if reset_priority && !st.is_restrained() {
                            if SetPriorityClass(h, NORMAL_PRIORITY_CLASS) != 0 {
                                ok = true;
                            }
                        }
                        if ok {
                            count += 1;
                        }
                        CloseHandle(h);
                    }
                }
            }
        }
        count
    }

    /// Returns top active user processes sorted by aggregate CPU% descending, then instance count
    pub fn get_top_active_apps(
        &self,
        cfg: &AppConfig,
        limit: usize,
    ) -> Vec<(String, usize, f32, Option<Rule>)> {
        let mut stats: HashMap<&str, (usize, f32)> = HashMap::with_capacity(32);
        for (&pid, st) in &self.procs {
            if pid <= 4 || pid == self.self_pid || st.name_lower.is_empty() {
                continue;
            }
            if st.is_critical_immune() && st.rule_idx == NO_RULE {
                continue;
            }
            let entry = stats.entry(st.name_lower.as_str()).or_insert((0, 0.0));
            entry.0 += 1;
            entry.1 += st.last_cpu_pct;
        }
        let mut list: Vec<(&str, usize, f32)> = stats
            .into_iter()
            .map(|(name, (cnt, cpu))| (name, cnt, cpu))
            .collect();

        list.sort_by(|a, b| {
            b.2.partial_cmp(&a.2)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| b.1.cmp(&a.1))
                .then_with(|| a.0.cmp(&b.0))
        });
        list.truncate(limit);
        list.into_iter()
            .map(|(name, cnt, cpu)| {
                let rule = cfg.find_rule_by_name(name).cloned();
                (name.to_string(), cnt, cpu, rule)
            })
            .collect()
    }

    /// Restores all currently restrained processes back to their original priority
    pub fn restore_all_restrained(&mut self) {
        for (&pid, state) in self.procs.iter_mut() {
            if state.is_restrained() {
                unsafe {
                    let h = OpenProcess(PROCESS_SET_INFORMATION, 0, pid);
                    if h != NULL && h != INVALID_HANDLE_VALUE {
                        let prio = if state.original_priority != 0 {
                            state.original_priority as DWORD
                        } else {
                            NORMAL_PRIORITY_CLASS
                        };
                        SetPriorityClass(h, prio);
                        CloseHandle(h);
                    }
                }
                state.set_restrained(false);
                state.over_quota_ms = 0;
            }
        }
        self.active_restrained_count = 0;
    }

    /// Single-syscall, zero-heap-allocation tick
    pub fn tick(&mut self, cfg: &AppConfig) {
        let now_ms = unsafe { GetTickCount64() };
        let now_rel_ms = self.rel_ms(now_ms);
        let delta_ms = now_ms.saturating_sub(self.last_tick_ms).max(1);
        let delta_ms_u16 = delta_ms.min(u16::MAX as u64) as u16;
        self.last_tick_ms = now_ms;
        self.tick_counter = self.tick_counter.wrapping_add(1);
        let cur_tick = self.tick_counter;

        let game_verify_this_tick = cfg.enforce_affinity_every_ticks > 0
            && (cur_tick % cfg.enforce_affinity_every_ticks == 0);

        // 1. Single NtQuerySystemInformation call for all processes (4 KB page-aligned sizing)
        let actual_snap_bytes: usize;
        unsafe {
            loop {
                let mut ret_len: ULONG = 0;
                let status = NtQuerySystemInformation(
                    SystemProcessInformation,
                    self.snapshot_buf.as_mut_ptr() as *mut c_void,
                    self.snapshot_buf.len() as ULONG,
                    &mut ret_len,
                );
                if status == STATUS_INFO_LENGTH_MISMATCH {
                    let needed = (ret_len as usize + 16384 + 4095) & !4095;
                    self.snapshot_buf.resize(needed.max(self.snapshot_buf.len() + 16384), 0);
                    continue;
                }
                if status < 0 {
                    return;
                }
                actual_snap_bytes = ret_len as usize;
                break;
            }
        }

        // Reclaim excess snapshot buffer capacity if a transient thread spike subsided
        if cur_tick % 30 == 0
            && actual_snap_bytes > 0
            && self.snapshot_buf.len() > actual_snap_bytes + 65536
        {
            let target = (actual_snap_bytes + 16384 + 4095) & !4095;
            self.snapshot_buf.truncate(target);
            self.snapshot_buf.shrink_to_fit();
        }

        // 2. Query Foreground Window PID (cached by HWND to avoid redundant syscalls)
        let fg_pid: u32 = unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd == NULL {
                self.last_fg_hwnd = NULL;
                self.last_fg_pid = 0;
                0
            } else if hwnd == self.last_fg_hwnd && self.procs.contains_key(&self.last_fg_pid) {
                self.last_fg_pid
            } else {
                let mut pid: DWORD = 0;
                GetWindowThreadProcessId(hwnd, &mut pid);
                self.last_fg_hwnd = hwnd;
                self.last_fg_pid = pid;
                pid
            }
        };

        // 3. First pass over snapshot buffer: update all processes, and only push active/managed
        //    processes into `self.items_buf` (eliminates ~90% of second-pass HashMap lookups)
        let wall_100ns = (delta_ms as f64) * 10_000.0;
        let total_capacity_100ns = wall_100ns * (self.num_cpus as f64);
        let mut total_busy_100ns: i64 = 0;

        self.items_buf.clear();
        let mut offset: usize = 0;

        unsafe {
            loop {
                let spi = &*(self.snapshot_buf.as_ptr().add(offset)
                    as *const SYSTEM_PROCESS_INFORMATION);
                let pid = spi.UniqueProcessId as u32;
                let parent_pid = spi.InheritedFromUniqueProcessId as u32;
                let total_time = spi.KernelTime.saturating_add(spi.UserTime);

                if pid != 0 {
                    let mut delta_time: i64 = 0;
                    let mut is_new = false;
                    let mut need_pass2 = false;

                    if let Some(existing) = self.procs.get_mut(&pid) {
                        if existing.create_time == spi.CreateTime {
                            delta_time = total_time.saturating_sub(existing.last_total_time);
                            existing.last_total_time = total_time;
                            existing.seen_tick = cur_tick;
                            if delta_time == 0 {
                                existing.last_cpu_pct = 0.0;
                                existing.over_quota_ms = 0;
                            }
                            need_pass2 = delta_time > 0
                                || existing.rule_idx != NO_RULE
                                || existing.is_restrained();
                        } else {
                            is_new = true;
                        }
                    } else {
                        is_new = true;
                    }

                    if is_new {
                        let name_lower = extract_unicode_name_lower(&spi.ImageName);
                        let is_crit = is_critical_system_process(&name_lower);
                        let rule_idx = lookup_rule(
                            cfg,
                            pid,
                            &name_lower,
                            is_crit,
                            &mut self.proc_paths,
                        );
                        need_pass2 = rule_idx != NO_RULE;
                        let flags = if is_crit { FLAG_CRITICAL_IMMUNE } else { 0 };
                        self.procs.insert(
                            pid,
                            ProcState {
                                create_time: spi.CreateTime,
                                last_total_time: total_time,
                                name_lower,
                                first_seen_rel_ms: now_rel_ms,
                                restrained_rel_ms: 0,
                                seen_tick: cur_tick,
                                last_cpu_pct: 0.0,
                                over_quota_ms: 0,
                                rule_idx,
                                original_priority: NORMAL_PRIORITY_CLASS as u16,
                                rule_applied: 0,
                                flags,
                            },
                        );
                    }

                    if pid != 4 && delta_time > 0 {
                        total_busy_100ns = total_busy_100ns.saturating_add(delta_time);
                    }

                    if pid == fg_pid
                        && pid != self.self_pid
                        && pid > 4
                        && self.last_user_fg_pid != pid
                    {
                        if let Some(st) = self.procs.get(&pid) {
                            if !st.name_lower.is_empty() && !st.is_critical_immune() {
                                self.last_user_fg_pid = pid;
                                self.last_user_fg_name.clear();
                                self.last_user_fg_name.push_str(st.name_lower.as_str());
                            }
                        }
                    }

                    if need_pass2 && pid > 4 && pid != self.self_pid {
                        self.items_buf.push(SnapItem {
                            pid,
                            parent_pid,
                            session_id: spi.SessionId,
                            base_priority: spi.BasePriority,
                            delta_time,
                        });
                    }
                }

                if spi.NextEntryOffset == 0 {
                    break;
                }
                offset += spi.NextEntryOffset as usize;
                if offset + std::mem::size_of::<SYSTEM_PROCESS_INFORMATION>()
                    > self.snapshot_buf.len()
                {
                    break;
                }
            }
        }

        let total_cpu_pct = if total_capacity_100ns > 0.0 {
            ((total_busy_100ns as f64 / total_capacity_100ns) * 100.0).clamp(0.0, 100.0) as f32
        } else {
            0.0
        };
        self.last_total_cpu_pct = total_cpu_pct;

        if cur_tick % 5 == 0 {
            self.procs.retain(|&pid, st| {
                let keep = st.seen_tick == cur_tick;
                if !keep && !self.proc_paths.is_empty() {
                    self.proc_paths.remove(&pid);
                }
                keep
            });
            if self.procs.capacity() > 384 && self.procs.len() < 256 {
                self.procs.shrink_to(256);
            }
        }

        let mut active_restrained = 0usize;
        let items_len = self.items_buf.len();

        for idx in 0..items_len {
            let item = self.items_buf[idx];
            let Some(state) = self.procs.get_mut(&item.pid) else {
                continue;
            };

            let proc_cpu_pct = if total_capacity_100ns > 0.0 && item.delta_time > 0 {
                ((item.delta_time as f64 / total_capacity_100ns) * 100.0) as f32
            } else {
                0.0
            };
            state.last_cpu_pct = proc_cpu_pct;

            if state.is_critical_immune() && state.rule_idx == NO_RULE {
                continue;
            }

            let rule: Option<&Rule> = if state.rule_idx != NO_RULE {
                cfg.rules.get(state.rule_idx as usize)
            } else {
                None
            };

            // (A) Per-process rule enforcement (affinity + priority)
            if cfg.affinity_enabled {
                if let Some(rule) = rule {
                    let age_ms = now_rel_ms.saturating_sub(state.first_seen_rel_ms) as u64;
                    if age_ms >= rule.delay_ms {
                        let need_check =
                            state.rule_applied == 0 || (rule.persist && game_verify_this_tick);

                        if need_check {
                            unsafe {
                                let h = OpenProcess(
                                    PROCESS_SET_INFORMATION | PROCESS_QUERY_LIMITED_INFORMATION,
                                    0,
                                    item.pid,
                                );
                                if h != NULL && h != INVALID_HANDLE_VALUE {
                                    let mut changed = false;

                                    if let Some(target_mask) = rule.affinity_mask {
                                        let mut cur_mask: usize = 0;
                                        let mut sys_mask: usize = 0;
                                        let got =
                                            GetProcessAffinityMask(h, &mut cur_mask, &mut sys_mask);
                                        let effective_target = if sys_mask != 0 {
                                            target_mask & sys_mask
                                        } else {
                                            target_mask
                                        };
                                        if effective_target != 0
                                            && (got == 0 || cur_mask != effective_target)
                                            && SetProcessAffinityMask(h, effective_target) != 0
                                        {
                                            changed = true;
                                        }
                                    }

                                    // While FairShare holds the process down, leave its priority alone.
                                    if let Some(prio) = rule.priority {
                                        if !state.is_restrained()
                                            && GetPriorityClass(h) != prio
                                            && SetPriorityClass(h, prio) != 0
                                        {
                                            changed = true;
                                        }
                                    }

                                    if changed && state.rule_applied == 0 {
                                        self.total_affinity_actions += 1;
                                        let ev = format!(
                                            "[{}] 应用规则 {} ({}) -> {}",
                                            now_hms(),
                                            state.name_lower,
                                            item.pid,
                                            rule.summary()
                                        );
                                        if self.recent_events.len() >= MAX_RECENT_EVENTS {
                                            self.recent_events.pop_front();
                                        }
                                        self.recent_events.push_back(ev);
                                    }
                                    state.rule_applied = state.rule_applied.saturating_add(1).min(2);
                                    CloseHandle(h);
                                } else {
                                    // Cannot open (protected/elevated): don't retry every tick.
                                    state.rule_applied = 2;
                                }
                            }
                        }
                    }
                }
            }

            if state.is_critical_immune() {
                continue;
            }

            // (B) FairShare Dynamic Background Scheduling (公平调度动态平衡)
            let fs_mode = rule.map_or(FairShareMode::Default, |r| r.fairshare);
            let force = fs_mode == FairShareMode::Force;
            // A rule that pins an explicit priority implies exemption unless `fs=force`.
            let rule_exempt = fs_mode == FairShareMode::Exempt
                || (fs_mode == FairShareMode::Default
                    && rule.map_or(false, |r| r.priority.is_some()));

            let is_fg = cfg.fairshare.exclude_foreground && item.pid == fg_pid;
            let is_fg_child = cfg.fairshare.exclude_foreground_children
                && fg_pid != 0
                && item.parent_pid == fg_pid;

            if state.is_restrained() {
                let held_ms = now_rel_ms.saturating_sub(state.restrained_rel_ms) as u64;
                let should_restore = !cfg.fairshare.enabled
                    || rule_exempt
                    || (!force && (is_fg || is_fg_child))
                    || (held_ms >= cfg.fairshare.min_restraint_ms
                        && proc_cpu_pct < cfg.fairshare.process_restore_pct);

                if should_restore {
                    unsafe {
                        let h = OpenProcess(PROCESS_SET_INFORMATION, 0, item.pid);
                        if h != NULL && h != INVALID_HANDLE_VALUE {
                            SetPriorityClass(h, state.original_priority as DWORD);
                            CloseHandle(h);
                        }
                    }
                    state.set_restrained(false);
                    state.over_quota_ms = 0;
                    let ev = format!(
                        "[{}] 恢复 {} ({}) 优先级",
                        now_hms(),
                        state.name_lower,
                        item.pid
                    );
                    if self.recent_events.len() >= MAX_RECENT_EVENTS {
                        self.recent_events.pop_front();
                    }
                    self.recent_events.push_back(ev);
                } else {
                    active_restrained += 1;
                }
            } else if cfg.fairshare.enabled {
                if total_cpu_pct >= cfg.fairshare.total_cpu_threshold_pct
                    && proc_cpu_pct >= cfg.fairshare.process_cpu_threshold_pct
                {
                    let excluded = rule_exempt
                        || (!force
                            && (is_fg
                                || is_fg_child
                                || (cfg.fairshare.exclude_services && item.session_id == 0)
                                || (cfg.fairshare.tame_only_normal && item.base_priority != 8)
                                || cfg
                                    .fairshare
                                    .exclusions
                                    .iter()
                                    .any(|ex| ex == &state.name_lower)));

                    if !excluded {
                        state.over_quota_ms = state.over_quota_ms.saturating_add(delta_ms_u16);
                        if (state.over_quota_ms as u64) >= cfg.fairshare.time_over_quota_ms {
                            unsafe {
                                let h = OpenProcess(
                                    PROCESS_SET_INFORMATION | PROCESS_QUERY_LIMITED_INFORMATION,
                                    0,
                                    item.pid,
                                );
                                if h != NULL && h != INVALID_HANDLE_VALUE {
                                    let orig = GetPriorityClass(h);
                                    if force
                                        || !cfg.fairshare.tame_only_normal
                                        || orig == NORMAL_PRIORITY_CLASS
                                    {
                                        if SetPriorityClass(h, BELOW_NORMAL_PRIORITY_CLASS) != 0 {
                                            state.set_restrained(true);
                                            state.restrained_rel_ms = now_rel_ms;
                                            state.original_priority = if orig != 0 {
                                                orig as u16
                                            } else {
                                                NORMAL_PRIORITY_CLASS as u16
                                            };
                                            active_restrained += 1;
                                            self.total_fairshare_actions += 1;
                                            let ev = format!(
                                                "[{}] 动态平衡调节 {} ({}, {:.1}%) -> 低于正常",
                                                now_hms(),
                                                state.name_lower,
                                                item.pid,
                                                proc_cpu_pct
                                            );
                                            if self.recent_events.len() >= MAX_RECENT_EVENTS {
                                                self.recent_events.pop_front();
                                            }
                                            self.recent_events.push_back(ev);
                                        }
                                    }
                                    CloseHandle(h);
                                }
                            }
                        }
                    } else {
                        state.over_quota_ms = 0;
                    }
                } else {
                    state.over_quota_ms = 0;
                }
            }
        }

        self.active_restrained_count = active_restrained;
    }
}

fn extract_unicode_name_lower(us: &UNICODE_STRING) -> CompactName {
    if us.Buffer.is_null() || us.Length == 0 {
        return CompactName::empty();
    }
    let len = (us.Length as usize) / 2;
    let slice = unsafe { std::slice::from_raw_parts(us.Buffer, len) };
    CompactName::from_utf16_lower(slice)
}

/// Finds the highest-priority rule matching this process (exact rules take precedence over globs).
/// Wildcard rules never apply to critical system processes; the full image path is only queried
/// (once, into `proc_paths`) when a full-path rule is evaluated.
fn lookup_rule(
    cfg: &AppConfig,
    pid: u32,
    name_lower: &str,
    is_critical: bool,
    proc_paths: &mut HashMap<u32, Box<str>>,
) -> u16 {
    if name_lower.is_empty() {
        return NO_RULE;
    }
    // 1. Exact process-name match first (0 path syscalls)
    for (i, r) in cfg.rules.iter().enumerate() {
        if r.is_exact_for(name_lower) {
            return (i as u16).min(NO_RULE - 1);
        }
    }
    // 2. Wildcard / path match in user-defined order
    for (i, r) in cfg.rules.iter().enumerate() {
        if is_critical && r.is_glob {
            continue;
        }
        let path_opt: Option<&str> = if r.match_path {
            Some(
                &**proc_paths.entry(pid).or_insert_with(|| {
                    process_image_path_lower(pid)
                        .unwrap_or_default()
                        .into_boxed_str()
                }),
            )
        } else {
            None
        };
        if r.matches(name_lower, path_opt) {
            return (i as u16).min(NO_RULE - 1);
        }
    }
    NO_RULE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proc_state_cache_line_size() {
        assert_eq!(std::mem::size_of::<CompactName>(), 24);
        assert_eq!(std::mem::size_of::<ProcState>(), 64);
    }
}
