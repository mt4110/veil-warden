// SPDX-License-Identifier: MIT
#![no_std]
/// One per-CPU u64 slot. No packet contents or identifying data cross the boundary.
pub const COUNTER_INDEX: u32 = 0;
pub const COUNTER_ENTRIES: u32 = 1;
pub const ALLOW: i32 = 1;

/// ABI v2 metadata is little endian (the build target is bpfel).
/// Address octets are in network order; port is host-valued, encoded little endian.
pub const ABI_VERSION: u16 = 2;
pub const EVENT_SIZE: usize = 56;
pub const TCP: u8 = 6;
pub const ACTION_ALLOWED: u8 = 0;
pub const HOOK_CONNECT4: u8 = 4;
pub const HOOK_CONNECT6: u8 = 6;
pub const RING_BYTES: u32 = 16 * 1024;
pub const STAT_ATTEMPTS: u32 = 0;
pub const STAT_EMITTED: u32 = 1;
pub const STAT_RING_DROPPED: u32 = 2;
pub const STAT_ENTRIES: u32 = 4;

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

/// Fixed byte key: network address octets, little-endian port, family, protocol.
pub type RuleKey = [u8; 20];
pub const POLICY_CAPACITY: u32 = 16;
pub const ACTION_DENIED: u8 = 1;
pub const STAT_DENIED: u32 = 3;
pub const ENFORCE: u32 = 1;
pub const DENY: i32 = 0;
/// Normalize mapped IPv6 destinations so an IPv4 rule cannot be bypassed.
pub fn rule_key(mut address: [u8; 16], mut family: u8, port: u16, protocol: u8) -> RuleKey {
    if family == 6 && address[..10] == [0; 10] && address[10..12] == [255; 2] {
        address = [
            address[12],
            address[13],
            address[14],
            address[15],
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ];
        family = 4;
    }
    let mut key = [0; 20];
    key[..16].copy_from_slice(&address);
    key[16..18].copy_from_slice(&port.to_le_bytes());
    key[18] = family;
    key[19] = protocol;
    key
}
/// Observe, unmatched destinations, and non-TCP traffic always pass.
pub fn verdict(mode: u32, protocol: u8, policy_id: u32) -> i32 {
    if mode == ENFORCE && protocol == TCP && policy_id != 0 {
        DENY
    } else {
        ALLOW
    }
}
#[cfg(test)]
mod policy_tests {
    use super::*;
    #[test]
    fn exact_key_and_mapped_address() {
        let mut a = [0; 16];
        a[..4].copy_from_slice(&[127, 0, 0, 1]);
        let k = rule_key(a, 4, 443, TCP);
        assert_eq!(&k[16..], &[187, 1, 4, 6]);
        assert_ne!(k, rule_key(a, 4, 444, TCP));
        assert_ne!(k, rule_key(a, 4, 443, 17));
        let mut mapped = [0; 16];
        mapped[10..12].copy_from_slice(&[255; 2]);
        mapped[12..].copy_from_slice(&a[..4]);
        assert_eq!(k, rule_key(mapped, 6, 443, TCP));
    }
    #[test]
    fn only_explicit_enforce_tcp_match_denies() {
        for mode in [0, 1, 2] {
            for protocol in [6, 17] {
                for id in [0, 1] {
                    assert_eq!(
                        verdict(mode, protocol, id),
                        if mode == 1 && protocol == 6 && id == 1 {
                            DENY
                        } else {
                            ALLOW
                        }
                    );
                }
            }
        }
    }
}
