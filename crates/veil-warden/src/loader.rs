// SPDX-License-Identifier: MIT
use crate::cli::Options;
use aya::{
    Ebpf,
    maps::PerCpuArray,
    programs::{CgroupAttachMode, CgroupSkb, CgroupSkbAttachType, links::FdLink},
};
use std::{
    fs::{self, File},
    io::{self, Write},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};
use veil_warden_common::COUNTER_INDEX;
const SCOPE: &str = "/sys/fs/cgroup/warden.slice/warden-test.slice";

pub fn run(options: Options) -> Result<(), Box<dyn std::error::Error>> {
    if fs::read_to_string("/etc/hostname")?.trim() != "veil-warden-sandbox" {
        return Err("M1 loader is restricted to the dedicated veil-warden-sandbox VM".into());
    }
    let release = fs::read_to_string("/proc/sys/kernel/osrelease")?;
    let mut parts = release.trim().split('.');
    let version = (
        parts.next().ok_or("kernel major missing")?.parse::<u32>()?,
        parts.next().ok_or("kernel minor missing")?.parse::<u32>()?,
    );
    if version < (5, 7) {
        return Err("FD-owned cgroup links require Linux >= 5.7".into());
    }
    if fs::canonicalize(SCOPE)? != std::path::Path::new(SCOPE) {
        return Err("test cgroup must not be a symlink".into());
    }
    // Install handlers before attaching. SIGKILL relies on kernel FD closure.
    let stop = Arc::new(AtomicBool::new(false));
    let int = signal_hook::flag::register(signal_hook::consts::SIGINT, stop.clone())?;
    let term = signal_hook::flag::register(signal_hook::consts::SIGTERM, stop.clone())?;
    let result = observe(options, &stop);
    signal_hook::low_level::unregister(int);
    signal_hook::low_level::unregister(term);
    result
}
fn observe(options: Options, stop: &AtomicBool) -> Result<(), Box<dyn std::error::Error>> {
    let instance = File::create("/run/veil-warden-counter.lock")?;
    instance.try_lock()?;
    let cgroup = File::open(SCOPE)?;
    let mut bpf = Ebpf::load_file(&options.object)?;
    let program: &mut CgroupSkb = bpf
        .program_mut("count_egress")
        .ok_or("count_egress program missing")?
        .try_into()?;
    program.load()?;
    // Aya 0.14 maps Single to flags=0. For BPF_LINK_CREATE the kernel
    // itself adds ALLOW_MULTI; passing that flag here would be EINVAL.
    // Linux >= 5.7 was checked above: never use legacy PROG_ATTACH.
    let id = program.attach(
        &cgroup,
        CgroupSkbAttachType::Egress,
        CgroupAttachMode::Single,
    )?;
    // Conversion fails for legacy PROG_ATTACH: fail closed rather than leave a persistent attachment.
    let link: FdLink = program.take_link(id)?.try_into()?;
    let counters: PerCpuArray<_, u64> =
        PerCpuArray::try_from(bpf.take_map("COUNTERS").ok_or("COUNTERS missing")?)?;
    let mut stdout = io::stdout().lock();
    writeln!(stdout, "attached scope={SCOPE} mode=observe link=fd")?;
    stdout.flush()?;
    let mut emitted = 0;
    let mut previous = 0;
    while !stop.load(Ordering::Relaxed) {
        let values = counters.get(&COUNTER_INDEX, 0)?;
        let total: u128 = values.iter().map(|v| u128::from(*v)).sum();
        writeln!(
            stdout,
            "packets={total} delta={}",
            total.saturating_sub(previous)
        )?;
        stdout.flush()?;
        previous = total;
        emitted += 1;
        if options.samples != 0 && emitted >= options.samples {
            break;
        }
        let deadline = Instant::now() + options.interval;
        while !stop.load(Ordering::Relaxed) && Instant::now() < deadline {
            std::thread::sleep(
                deadline
                    .saturating_duration_since(Instant::now())
                    .min(std::time::Duration::from_millis(20)),
            );
        }
    }
    drop(link);
    writeln!(stdout, "detached")?;
    Ok(())
}
