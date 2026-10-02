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
mod policy;
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
mod process;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|s| s == "policy") {
        #[cfg(target_os = "linux")]
        return policy::control::client(&args[1..]);
        #[cfg(not(target_os = "linux"))]
        return Err("policy control requires the dedicated Linux VM".into());
    }
    let Some(options) = cli::parse(args.into_iter())? else {
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
            enforce,
            initial_rule,
        } = options;
        let _ = (
            object,
            interval,
            samples,
            mode,
            duration,
            reader_delay,
            enforce,
            initial_rule,
        );
        Err("eBPF loading requires the dedicated Linux VM".into())
    }
}
