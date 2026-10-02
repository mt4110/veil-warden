// SPDX-License-Identifier: MIT
use veil_warden_common::*;
#[derive(Debug, PartialEq, Eq)]
pub enum DecodeError {
    Length,
    Version,
    Value,
}
pub fn decode(bytes: &[u8]) -> Result<ConnectEvent, DecodeError> {
    if bytes.len() != EVENT_SIZE {
        return Err(DecodeError::Length);
    }
    // Exact-size validation above makes these copies safe without pointer casts or Pod.
    let u64_at = |i| u64::from_le_bytes(bytes[i..i + 8].try_into().unwrap());
    let u32_at = |i| u32::from_le_bytes(bytes[i..i + 4].try_into().unwrap());
    let u16_at = |i| u16::from_le_bytes(bytes[i..i + 2].try_into().unwrap());
    let event = ConnectEvent {
        timestamp_ns: u64_at(0),
        cgroup_id: u64_at(8),
        tgid: u32_at(16),
        tid: u32_at(20),
        address: bytes[24..40].try_into().unwrap(),
        port: u16_at(40),
        abi_version: u16_at(42),
        family: bytes[44],
        protocol: bytes[45],
        action: bytes[46],
        hook: bytes[47],
        policy_id: u32_at(48),
        reserved: u32_at(52),
    };
    if event.abi_version != ABI_VERSION {
        return Err(DecodeError::Version);
    }
    if !matches!(event.family, 4 | 6)
        || event.hook != event.family
        || event.protocol != TCP
        || !matches!(
            (event.action, event.policy_id),
            (ACTION_ALLOWED, 0) | (ACTION_DENIED, 1..=u32::MAX)
        )
        || event.reserved != 0
        || event.tgid == 0
        || event.tid == 0
        || event.cgroup_id == 0
        || (event.family == 4 && event.address[4..].iter().any(|v| *v != 0))
    {
        return Err(DecodeError::Value);
    }
    Ok(event)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> [u8; 56] {
        let mut b = [0; 56];
        b[0..8].copy_from_slice(&123u64.to_le_bytes());
        b[8..16].copy_from_slice(&789u64.to_le_bytes());
        b[16..20].copy_from_slice(&100u32.to_le_bytes());
        b[20..24].copy_from_slice(&101u32.to_le_bytes());
        b[24..28].copy_from_slice(&[127, 0, 0, 1]);
        b[40..42].copy_from_slice(&443u16.to_le_bytes());
        b[42..44].copy_from_slice(&ABI_VERSION.to_le_bytes());
        b[44] = 4;
        b[45] = 6;
        b[47] = 4;
        b
    }
    #[test]
    fn wire_endianness_and_families() {
        let b = fixture();
        let e = decode(&b).unwrap();
        assert_eq!((e.port, e.tgid, e.tid), (443, 100, 101));
        assert_eq!(&e.address[..4], &[127, 0, 0, 1]);
        let mut v6 = b;
        v6[24..40].copy_from_slice(&[0x20, 1, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);
        v6[44] = 6;
        v6[47] = 6;
        assert!(decode(&v6).is_ok());
    }
    #[test]
    fn deny_requires_rule_id() {
        let mut b = fixture();
        b[46] = ACTION_DENIED;
        assert_eq!(decode(&b), Err(DecodeError::Value));
        b[48..52].copy_from_slice(&7u32.to_le_bytes());
        assert_eq!(decode(&b).unwrap().policy_id, 7);
        b[46] = ACTION_ALLOWED;
        assert_eq!(decode(&b), Err(DecodeError::Value));
    }
    #[test]
    fn invalid_records_are_rejected() {
        let b = fixture();
        for size in 0..56 {
            assert_eq!(decode(&b[..size]), Err(DecodeError::Length));
        }
        assert_eq!(decode(&[0; 57]), Err(DecodeError::Length));
        for (offset, value) in [
            (42, 1),
            (44, 5),
            (45, 17),
            (46, 1),
            (47, 6),
            (48, 1),
            (52, 1),
            (28, 1),
            (16, 0),
            (20, 0),
            (8, 0),
        ] {
            let mut bad = b;
            bad[offset] = value;
            if offset == 8 {
                bad[8..16].fill(0);
            }
            assert!(decode(&bad).is_err(), "offset {offset}");
        }
    }
}
