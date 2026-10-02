// SPDX-License-Identifier: MIT
#![no_std]
#![no_main]
mod connect;
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}
