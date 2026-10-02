// SPDX-License-Identifier: MIT
use crate::{
    cli::Options,
    decode,
    output::{self, Message, QUEUE_CAPACITY, Snapshot},
};
use aya::{
    Ebpf,
    maps::{MapData, PerCpuArray, RingBuf},
    programs::{CgroupAttachMode, CgroupSockAddr, links::FdLink},
};
use std::{
    fs::File,
    io::{self, Write},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
use tokio::{io::unix::AsyncFd, sync::mpsc};
use veil_warden_common::{STAT_ATTEMPTS, STAT_EMITTED, STAT_RING_DROPPED};
type Error = Box<dyn std::error::Error>;
type StatsMap = PerCpuArray<MapData, u64>;

pub fn run(options: Options, stop: &AtomicBool) -> Result<(), Error> {
    tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()?
        .block_on(observe(options, stop))
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
async fn observe(options: Options, stop: &AtomicBool) -> Result<(), Error> {
    let instance = File::create("/run/veil-warden-connect.lock")?;
    instance.try_lock()?;
    let cgroup = File::open("/sys/fs/cgroup/warden.slice/warden-test.slice")?;
    let mut bpf = Ebpf::load_file(&options.object)?;
    // All maps and both programs must succeed before reporting ready.
    let stats_map = StatsMap::try_from(bpf.take_map("STATS").ok_or("STATS missing")?)?;
    let ring = RingBuf::try_from(bpf.take_map("EVENTS").ok_or("EVENTS missing")?)?;
    let mut ring = AsyncFd::new(ring)?;
    let link4 = attach(&mut bpf, &cgroup, "monitor_connect4")?;
    let link6 = attach(&mut bpf, &cgroup, "monitor_connect6")?;
    let (tx, rx) = mpsc::channel(QUEUE_CAPACITY);
    let worker = std::thread::Builder::new()
        .name("connect-output".into())
        .spawn(move || output::worker(rx))?;
    let result = async {
        writeln!(io::stdout(),"attached mode=connect scope=/warden.slice/warden-test.slice links=2 observation=attempt_only")?;
        io::stdout().flush()?;
        let start=Instant::now(); let mut last_stats=start;
        let mut tick=tokio::time::interval(Duration::from_millis(20));
        let mut stats=Snapshot::default();
        while !stop.load(Ordering::Relaxed) && (options.duration.is_zero() || start.elapsed()<options.duration) {
            tokio::select! {
                ready=ring.readable_mut() => {
                    let mut guard=ready?;
                    if !options.reader_delay.is_zero() { tokio::time::sleep(options.reader_delay).await; }
                    if drain(guard.get_inner_mut(),&tx,&mut stats)? { guard.clear_ready(); }
                },
                _=tick.tick() => {
                    if last_stats.elapsed() >= options.interval {
                        let message=Message::Stats(snapshot(&stats_map,stats)?,false);
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
                tx.send(Message::Stats(snapshot(&stats_map, stats)?, true))
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
    writeln!(io::stdout(), "detached mode=connect")?;
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
