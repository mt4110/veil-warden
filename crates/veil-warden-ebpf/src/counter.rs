// SPDX-License-Identifier: MIT
use aya_ebpf::{
    macros::{cgroup_skb, map},
    maps::PerCpuArray,
    programs::SkBuffContext,
};
use veil_warden_common::{ALLOW, COUNTER_ENTRIES, COUNTER_INDEX};

#[map]
static COUNTERS: PerCpuArray<u64> = PerCpuArray::with_max_entries(COUNTER_ENTRIES, 0);

#[cgroup_skb(egress)]
pub fn count_egress(_ctx: SkBuffContext) -> i32 {
    if let Some(pointer) = COUNTERS.get_ptr_mut(COUNTER_INDEX) {
        // SAFETY: lookup returned the current CPU's aligned u64 map slot.
        // The pointer lives for this non-sleepable invocation; wrapping avoids panic.
        unsafe {
            *pointer = (*pointer).wrapping_add(1);
        }
    }
    // Map lookup failure must never turn observation into enforcement.
    ALLOW
}
