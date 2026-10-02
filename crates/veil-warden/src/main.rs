// SPDX-License-Identifier: MIT
mod cli;
#[cfg(target_os = "linux")]
mod loader;
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
        } = options;
        let _ = (object, interval, samples);
        Err("eBPF loading requires the dedicated Linux VM".into())
    }
}
