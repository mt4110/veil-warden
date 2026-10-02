// SPDX-License-Identifier: MIT
use aya_ebpf::{
    helpers::{bpf_get_current_cgroup_id, bpf_get_current_pid_tgid, bpf_ktime_get_ns},
    macros::{cgroup_sock_addr, map},
    maps::{PerCpuArray, RingBuf},
    programs::SockAddrContext,
};
use veil_warden_common::*;
#[map]
static EVENTS: RingBuf = RingBuf::with_byte_size(RING_BYTES, 0);
#[map]
static STATS: PerCpuArray<u64> = PerCpuArray::with_max_entries(STAT_ENTRIES, 0);

#[inline(always)]
fn increment(index: u32) {
    if let Some(p) = STATS.get_ptr_mut(index) {
        // SAFETY: lookup returns this CPU's aligned slot for this invocation.
        unsafe {
            *p = (*p).wrapping_add(1);
        }
    }
}
#[inline(always)]
fn emit(ctx: &SockAddrContext, family: u8) {
    // SAFETY: connect hooks supply a valid bpf_sock_addr context; only read fields.
    let protocol = unsafe { (*ctx.sock_addr).protocol };
    if protocol != u32::from(TCP) {
        return;
    }
    increment(STAT_ATTEMPTS);
    let Some(mut slot) = EVENTS.reserve::<ConnectEvent>(0) else {
        increment(STAT_RING_DROPPED);
        return;
    };
    let id = bpf_get_current_pid_tgid();
    let mut address = [0; 16];
    // Context address fields contain network bytes stored in native integers.
    if family == 4 {
        let value = unsafe { (*ctx.sock_addr).user_ip4 };
        let bytes = u32::from_be(value).to_be_bytes();
        address[0] = bytes[0];
        address[1] = bytes[1];
        address[2] = bytes[2];
        address[3] = bytes[3];
    } else {
        let words = unsafe { (*ctx.sock_addr).user_ip6 };
        for i in 0..4 {
            let bytes = u32::from_be(words[i]).to_be_bytes();
            address[i * 4] = bytes[0];
            address[i * 4 + 1] = bytes[1];
            address[i * 4 + 2] = bytes[2];
            address[i * 4 + 3] = bytes[3];
        }
    }
    let port = u16::from_be(unsafe { (*ctx.sock_addr).user_port } as u16);
    slot.write(ConnectEvent {
        timestamp_ns: unsafe { bpf_ktime_get_ns() },
        cgroup_id: unsafe { bpf_get_current_cgroup_id() },
        tgid: (id >> 32) as u32,
        tid: id as u32,
        address,
        port,
        abi_version: ABI_VERSION,
        family,
        protocol: TCP,
        action: ACTION_ALLOWED,
        hook: family,
        policy_id: 0,
        reserved: 0,
    });
    slot.submit(0);
    increment(STAT_EMITTED);
}
#[cgroup_sock_addr(connect4)]
pub fn monitor_connect4(ctx: SockAddrContext) -> i32 {
    emit(&ctx, 4);
    ALLOW
}
#[cfg(not(feature = "partial-fixture"))]
#[cgroup_sock_addr(connect6)]
pub fn monitor_connect6(ctx: SockAddrContext) -> i32 {
    emit(&ctx, 6);
    ALLOW
}
