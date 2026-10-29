use crate::model::{Disk, Health, Memory, SCHEMA_VERSION, Snapshot, now_unix};
use std::{
    collections::VecDeque,
    ffi::{CStr, CString},
    mem, ptr,
};
use sysinfo::{MemoryRefreshKind, System};

pub fn disks(external: &std::path::Path) -> Vec<Disk> {
    vec![
        disk_at("Internal", "/System/Volumes/Data"),
        disk_at("Media", &external.to_string_lossy()),
    ]
}

fn disk_at(name: &str, path: &str) -> Disk {
    let Ok(cpath) = CString::new(path) else {
        return Disk::missing(name, path);
    };
    // SAFETY: caller-owned output, valid C string; statfs refreshes the selected filesystem.
    unsafe {
        let mut stats: libc::statfs = mem::zeroed();
        if libc::statfs(cpath.as_ptr(), &mut stats) != 0 {
            return Disk::missing(name, path);
        }
        // An existing /Volumes/Media directory is not proof the external disk is mounted.
        if CStr::from_ptr(stats.f_mntonname.as_ptr()).to_string_lossy() != path {
            return Disk::missing(name, path);
        }
        Disk::mounted(
            name,
            path,
            stats.f_blocks.saturating_mul(stats.f_bsize as u64),
            stats.f_bfree.saturating_mul(stats.f_bsize as u64),
            stats.f_bavail.saturating_mul(stats.f_bsize as u64),
        )
    }
}

fn pressure() -> String {
    let mut value: libc::c_int = 0;
    let mut len = mem::size_of_val(&value);
    // SAFETY: correctly sized output and nul-terminated static sysctl name.
    let result = unsafe {
        libc::sysctlbyname(
            c"kern.memorystatus_vm_pressure_level".as_ptr(),
            (&mut value as *mut libc::c_int).cast(),
            &mut len,
            ptr::null_mut(),
            0,
        )
    };
    if result != 0 {
        return "unknown".into();
    }
    match value {
        1 => "normal",
        2 => "warning",
        4 => "critical",
        _ => "unknown",
    }
    .into()
}

fn awake_seconds() -> u64 {
    // CLOCK_UPTIME_RAW advances while awake, excluding system sleep.
    // SAFETY: valid timespec pointer and a macOS-supported clock identifier.
    unsafe {
        let mut time: libc::timespec = mem::zeroed();
        if libc::clock_gettime(libc::CLOCK_UPTIME_RAW, &mut time) == 0 {
            time.tv_sec.max(0) as u64
        } else {
            0
        }
    }
}

pub struct Sampler {
    system: System,
    warm: bool,
    history: VecDeque<f32>,
}
impl Default for Sampler {
    fn default() -> Self {
        Self {
            system: System::new(),
            warm: false,
            history: VecDeque::new(),
        }
    }
}
impl Sampler {
    pub fn sample(
        &mut self,
        disks: Vec<Disk>,
        disks_sampled_at_unix: u64,
        health: Health,
    ) -> Snapshot {
        self.system.refresh_cpu_usage();
        self.system
            .refresh_memory_specifics(MemoryRefreshKind::everything());
        let cpu = self.warm.then(|| self.system.global_cpu_usage());
        self.warm = true;
        if let Some(value) = cpu {
            self.history.push_back(value);
            if self.history.len() > 60 {
                self.history.pop_front();
            }
        }
        let total = self.system.total_memory();
        let used = self.system.used_memory();
        Snapshot {
            schema_version: SCHEMA_VERSION,
            sampled_at_unix: now_unix(),
            boot_time_unix: System::boot_time(),
            collector_pid: std::process::id(),
            hostname: System::host_name().unwrap_or_else(|| "Mac".into()),
            uptime_seconds: System::uptime(),
            awake_seconds: awake_seconds(),
            cpu_percent: cpu,
            cpu_history: self.history.iter().copied().collect(),
            memory: Memory {
                total_bytes: total,
                used_bytes: used,
                available_bytes: self.system.available_memory(),
                used_percent: if total > 0 {
                    used as f64 / total as f64 * 100.0
                } else {
                    0.0
                },
                swap_used_bytes: self.system.used_swap(),
                pressure: pressure(),
            },
            disks,
            disks_sampled_at_unix,
            health,
        }
    }
}
