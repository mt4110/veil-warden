// SPDX-License-Identifier: MIT
use crate::{
    cli::Options,
    decode,
    output::{self, Message, QUEUE_CAPACITY, Snapshot},
};
use aya::{
    Ebpf,
    maps::{Array, HashMap, MapData, PerCpuArray, RingBuf},
    programs::{CgroupAttachMode, CgroupSockAddr, links::FdLink},
};
use std::{
    fs::File,
    io::{self, Write},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::{io::unix::AsyncFd, sync::mpsc};
use veil_warden_common::{ENFORCE, STAT_ATTEMPTS, STAT_DENIED, STAT_EMITTED, STAT_RING_DROPPED};
type Error = Box<dyn std::error::Error>;
type StatsMap = PerCpuArray<MapData, u64>;

pub fn run(options: Options, stop: &Arc<AtomicBool>) -> Result<(), Error> {
    let (mut ui, mut commands) = if options.tui {
        let (ui, commands) = crate::tui::runtime::Ui::start(options.enforce, stop.clone())?;
        (Some(ui), Some(commands))
    } else {
        (None, None)
    };
    let result = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()?
        .block_on(observe(options, stop, ui.as_ref(), &mut commands));
    let frontend = if let Some(ui) = ui.as_mut() {
        if let Err(error) = &result {
            ui.state(|s| s.error = Some(error.to_string()))?;
        }
        ui.finish()
    } else {
        Ok(())
    };
    match (result, frontend) {
        (Err(backend), Err(frontend)) => Err(format!("{backend}; TUI: {frontend}").into()),
        (Err(error), _) => Err(error),
        (_, Err(error)) => Err(error.into()),
        _ => Ok(()),
    }
}
fn attach(bpf: &mut Ebpf, cgroup: &File, name: &str) -> Result<FdLink, Error> {
    let p: &mut CgroupSockAddr = bpf
        .program_mut(name)
        .ok_or("connect program missing")?
        .try_into()?;
    p.load()?;
    // flags=0: the Linux link-create path adds ALLOW_MULTI itself, as in M1.
    let id = p.attach(cgroup, CgroupAttachMode::Single)?;
    Ok(p.take_link(id)?.try_into()?)
}
fn snapshot(map: &StatsMap, mut s: Snapshot) -> Result<Snapshot, Error> {
    let sum = |index| -> Result<u128, Error> {
        Ok(map.get(&index, 0)?.iter().map(|v| u128::from(*v)).sum())
    };
    s.attempted = sum(STAT_ATTEMPTS)?;
    s.denied = sum(STAT_DENIED)?;
    s.emitted = sum(STAT_EMITTED)?;
    s.ring_dropped = sum(STAT_RING_DROPPED)?;
    Ok(s)
}
fn drain(
    ring: &mut RingBuf<MapData>,
    tx: &mpsc::Sender<Message>,
    stats: &mut Snapshot,
) -> Result<bool, Error> {
    // Yield between batches even under a continuous producer load.
    for _ in 0..128 {
        let Some(item) = ring.next() else {
            return Ok(true);
        };
        match decode::decode(&item) {
            Ok(event) => {
                stats.decoded += 1;
                queue_event(tx, event, stats)?;
            }
            Err(_) => stats.decode_errors += 1,
        }
    }
    Ok(false)
}
fn queue_event(
    tx: &mpsc::Sender<Message>,
    event: veil_warden_common::ConnectEvent,
    stats: &mut Snapshot,
) -> Result<(), Error> {
    match tx.try_send(Message::Event(event)) {
        Ok(()) => Ok(()),
        Err(mpsc::error::TrySendError::Full(_)) => {
            stats.queue_dropped += 1;
            Ok(())
        }
        Err(mpsc::error::TrySendError::Closed(_)) => Err("output worker stopped".into()),
    }
}
async fn observe(
    options: Options,
    stop: &AtomicBool,
    ui: Option<&crate::tui::runtime::Ui>,
    commands: &mut Option<mpsc::Receiver<crate::tui::Command>>,
) -> Result<(), Error> {
    let instance = File::create("/run/veil-warden-connect.lock")?;
    instance.try_lock()?;
    let cgroup = File::open("/sys/fs/cgroup/warden.slice/warden-test.slice")?;
    if let Some(target) = options.scan_argv {
        let summary = crate::scan::run(target);
        if let Some(ui) = ui {
            ui.state(|s| s.scan = Some(summary))?;
        } else {
            writeln!(
                io::stdout(),
                "scan pid={} start_ticks={} {summary}",
                target.pid,
                target.start_ticks
            )?;
        }
    }
    let mut bpf = Ebpf::load_file(&options.object)?;
    // All maps and both programs must succeed before reporting ready.
    let stats_map = StatsMap::try_from(bpf.take_map("STATS").ok_or("STATS missing")?)?;
    let ring = RingBuf::try_from(bpf.take_map("EVENTS").ok_or("EVENTS missing")?)?;
    let mut ring = AsyncFd::new(ring)?;
    let mut mode = Array::<_, u32>::try_from(bpf.take_map("MODE").ok_or("MODE missing")?)?;
    let rules = HashMap::try_from(
        bpf.take_map("DENY_DESTINATIONS")
            .ok_or("DENY_DESTINATIONS missing")?,
    )?;
    let mut policy =
        crate::policy::control::Controller::new(rules, options.enforce, options.initial_rule)?;
    mode.set(0, if options.enforce { ENFORCE } else { 0 }, 0)?;
    let link4 = attach(&mut bpf, &cgroup, "monitor_connect4")?;
    let link6 = attach(&mut bpf, &cgroup, "monitor_connect6")?;
    let (tx, rx) = mpsc::channel(QUEUE_CAPACITY);
    let worker = if let Some(ui) = ui {
        let rows = ui.rows.clone();
        let dropped = ui.dropped.clone();
        std::thread::Builder::new()
            .name("connect-names".into())
            .spawn(move || crate::tui::runtime::worker(rx, rows, dropped))?
    } else {
        std::thread::Builder::new()
            .name("connect-output".into())
            .spawn(move || output::worker(rx))?
    };
    let result = async {
        if let Some(ui)=ui {let rules=policy.rules()?;ui.state(|s|{s.ready=true;s.rules=rules;})?;} else {
        writeln!(io::stdout(),"attached mode=connect scope=/warden.slice/warden-test.slice links=2 observation=attempt_only policy_mode={}",if options.enforce {"enforce"}else{"observe"})?;
        io::stdout().flush()?; }
        let start=Instant::now(); let mut last_stats=start;
        let mut tick=tokio::time::interval(Duration::from_millis(20));
        let mut stats=Snapshot::default();
        while !stop.load(Ordering::Relaxed) && (options.duration.is_zero() || start.elapsed()<options.duration) {
            tokio::select! {
                accepted=policy.listener.accept()=>{let (stream,_)=accepted?;policy.serve(stream).await?;},
                ready=ring.readable_mut() => {
                    let mut guard=ready?;
                    if !options.reader_delay.is_zero() { tokio::time::sleep(options.reader_delay).await; }
                    if drain(guard.get_inner_mut(),&tx,&mut stats)? { guard.clear_ready(); }
                },
                _=tick.tick() => {
                    if let (Some(ui),Some(commands))=(ui,commands.as_mut()) {
                        if commands.is_closed(){return Err::<Snapshot,Error>("TUI input stopped".into());}
                        if let Ok(command)=commands.try_recv() {
                            let result=match command {
                                crate::tui::Command::Add(key)=>policy.add(key).map(|id|format!("拒否 ID={id} {} TCP",crate::policy::destination(&key))),
                                crate::tui::Command::Remove(key)=>policy.remove(key).map(|()|format!("解除 {} TCP",crate::policy::destination(&key))),
                            };
                            let reply=result.map_err(|e|format!("{e:?}"));
                            let rules=policy.rules()?;ui.state(|s|{s.reply=Some(reply);s.rules=rules;})?;
                        }
                    }

                    if last_stats.elapsed() >= options.interval {
                        let current=snapshot(&stats_map,stats)?;
                        if let Some(ui)=ui {let rules=policy.rules()?;ui.state(|s|{s.stats=current;s.rules=rules;})?;}
                        let message=Message::Stats(current,false);
                        match tx.try_send(message) {
                            Ok(()) | Err(mpsc::error::TrySendError::Full(_)) => {},
                            Err(mpsc::error::TrySendError::Closed(_)) => return Err::<Snapshot,Error>("output worker stopped".into()),
                        }
                        last_stats=Instant::now();
                    }
                },
            }
        }
        Ok(stats)
    }.await;
    // Even a receive/output error or partial startup drops both owned links.
    drop(link6);
    drop(link4);
    let result = match result {
        Ok(mut stats) => {
            async {
                while !drain(ring.get_mut(), &tx, &mut stats)? {
                    tokio::task::yield_now().await;
                }
                let final_stats = snapshot(&stats_map, stats)?;
                if let Some(ui) = ui {
                    ui.state(|s| s.stats = final_stats)?;
                }
                tx.send(Message::Stats(final_stats, true))
                    .await
                    .map_err(|_| "output worker stopped")?;
                Ok::<(), Error>(())
            }
            .await
        }
        Err(error) => Err(error),
    };
    drop(tx);
    let output_result = worker.join().map_err(|_| "output worker panicked")?;
    result?;
    output_result?;
    if ui.is_none() {
        writeln!(io::stdout(), "detached mode=connect")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn full_queue_is_counted_and_closed_queue_is_an_error() {
        let (tx, rx) = mpsc::channel(1);
        let mut s = Snapshot::default();
        let e = veil_warden_common::ConnectEvent {
            timestamp_ns: 0,
            cgroup_id: 1,
            tgid: 1,
            tid: 1,
            address: [0; 16],
            port: 0,
            abi_version: 1,
            family: 4,
            protocol: 6,
            action: 0,
            hook: 4,
            policy_id: 0,
            reserved: 0,
        };
        queue_event(&tx, e, &mut s).unwrap();
        queue_event(&tx, e, &mut s).unwrap();
        assert_eq!(s.queue_dropped, 1);
        assert_eq!(tx.capacity(), 0);
        drop(rx);
        assert!(queue_event(&tx, e, &mut s).is_err());
        assert_eq!(s.queue_dropped, 1);
    }
}
