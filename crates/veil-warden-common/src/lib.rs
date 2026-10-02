// SPDX-License-Identifier: MIT
#![no_std]
/// One per-CPU u64 slot. No packet contents or identifying data cross the boundary.
pub const COUNTER_INDEX: u32 = 0;
pub const COUNTER_ENTRIES: u32 = 1;
pub const ALLOW: i32 = 1;

/// ABI v1 metadata is little endian (the build target is bpfel).
/// Address octets are in network order; port is host-valued, encoded little endian.
pub const ABI_VERSION: u16 = 1;
pub const EVENT_SIZE: usize = 56;
pub const TCP: u8 = 6;
pub const ACTION_ALLOWED: u8 = 0;
pub const HOOK_CONNECT4: u8 = 4;
pub const HOOK_CONNECT6: u8 = 6;
pub const RING_BYTES: u32 = 16 * 1024;
pub const STAT_ATTEMPTS: u32 = 0;
pub const STAT_EMITTED: u32 = 1;
pub const STAT_RING_DROPPED: u32 = 2;
pub const STAT_ENTRIES: u32 = 3;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConnectEvent {
    pub timestamp_ns: u64,
    pub cgroup_id: u64,
    pub tgid: u32,
    pub tid: u32,
    pub address: [u8; 16],
    pub port: u16,
    pub abi_version: u16,
    pub family: u8,
    pub protocol: u8,
    pub action: u8,
    pub hook: u8,
    pub policy_id: u32,
    pub reserved: u32,
}
// Both compilers check these; no hidden padding can be copied to userspace.
const _: () = {
    assert!(core::mem::size_of::<ConnectEvent>() == EVENT_SIZE);
    assert!(core::mem::align_of::<ConnectEvent>() == 8);
    assert!(core::mem::offset_of!(ConnectEvent, timestamp_ns) == 0);
    assert!(core::mem::offset_of!(ConnectEvent, cgroup_id) == 8);
    assert!(core::mem::offset_of!(ConnectEvent, tgid) == 16);
    assert!(core::mem::offset_of!(ConnectEvent, tid) == 20);
    assert!(core::mem::offset_of!(ConnectEvent, address) == 24);
    assert!(core::mem::offset_of!(ConnectEvent, port) == 40);
    assert!(core::mem::offset_of!(ConnectEvent, abi_version) == 42);
    assert!(core::mem::offset_of!(ConnectEvent, family) == 44);
    assert!(core::mem::offset_of!(ConnectEvent, protocol) == 45);
    assert!(core::mem::offset_of!(ConnectEvent, action) == 46);
    assert!(core::mem::offset_of!(ConnectEvent, hook) == 47);
    assert!(core::mem::offset_of!(ConnectEvent, policy_id) == 48);
    assert!(core::mem::offset_of!(ConnectEvent, reserved) == 52);
};
