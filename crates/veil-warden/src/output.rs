// SPDX-License-Identifier: MIT
#[cfg(target_os = "linux")]
use crate::process;
#[cfg(target_os = "linux")]
use std::{
    io::{self, Write},
    net::{Ipv4Addr, Ipv6Addr},
};
#[cfg(target_os = "linux")]
use tokio::sync::mpsc::Receiver;
#[cfg(target_os = "linux")]
use veil_warden_common::{ConnectEvent, RING_BYTES};
pub const QUEUE_CAPACITY: usize = 128;
#[derive(Default, Clone, Copy)]
pub struct Snapshot {
    pub attempted: u128,
    pub denied: u128,
    pub emitted: u128,
    pub ring_dropped: u128,
    pub decoded: u64,
    pub decode_errors: u64,
    pub queue_dropped: u64,
}
#[cfg(target_os = "linux")]
pub enum Message {
    Event(ConnectEvent),
    Stats(Snapshot, bool),
}
#[cfg(target_os = "linux")]
pub fn worker(mut rx: Receiver<Message>) -> io::Result<()> {
    let mut displayed = 0u64;
    while let Some(message) = rx.blocking_recv() {
        // Do not hold stdout while waiting: startup/final control messages share it.
        let mut stdout = io::stdout().lock();
        match message {
            Message::Event(e) => {
                let (comm, status) = process::comm(e.tgid);
                let destination = if e.family == 4 {
                    format!(
                        "{}:{}",
                        Ipv4Addr::new(e.address[0], e.address[1], e.address[2], e.address[3]),
                        e.port
                    )
                } else {
                    format!("[{}]:{}", Ipv6Addr::from(e.address), e.port)
                };
                writeln!(
                    stdout,
                    "attempt family={} tgid={} tid={} cgroup={} timestamp_ns={} destination={} protocol=tcp decision={} policy_id={} connection_result=unknown comm={} comm_status={}",
                    e.family,
                    e.tgid,
                    e.tid,
                    e.cgroup_id,
                    e.timestamp_ns,
                    destination,
                    if e.action == veil_warden_common::ACTION_DENIED {
                        "deny"
                    } else {
                        "allow"
                    },
                    e.policy_id,
                    comm,
                    status
                )?;
                displayed += 1;
            }
            Message::Stats(s, final_stats) => writeln!(
                stdout,
                "stats final={} attempted={} denied={} emitted={} ring_dropped={} decoded={} decode_errors={} queue_dropped={} displayed={} queue_capacity={} ring_bytes={}",
                final_stats,
                s.attempted,
                s.denied,
                s.emitted,
                s.ring_dropped,
                s.decoded,
                s.decode_errors,
                s.queue_dropped,
                displayed,
                QUEUE_CAPACITY,
                RING_BYTES
            )?,
        }
        stdout.flush()?;
    }
    Ok(())
}
