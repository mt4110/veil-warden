// SPDX-License-Identifier: MIT
mod cli;
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
mod decode;
#[cfg(target_os = "linux")]
mod events;
#[cfg(target_os = "linux")]
mod loader;
#[cfg(target_os = "linux")]
mod output;
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
mod process;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Some(options) = cli::parse(std::env::args().skip(1))? else {
        return Ok(());
    };
    #[cfg(target_os = "linux")]
    return loader::run(options);
    #[cfg(not(target_os = "linux"))]
    {
        let cli::Options {
            object,
            interval,
            samples,
            mode,
            duration,
            reader_delay,
        } = options;
        let _ = (object, interval, samples, mode, duration, reader_delay);
        Err("eBPF loading requires the dedicated Linux VM".into())
    }
}
