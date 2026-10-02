// SPDX-License-Identifier: MIT
#![no_std]
/// One per-CPU u64 slot. No packet contents or identifying data cross the boundary.
pub const COUNTER_INDEX: u32 = 0;
pub const COUNTER_ENTRIES: u32 = 1;
pub const ALLOW: i32 = 1;
