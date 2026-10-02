// SPDX-License-Identifier: MIT
use std::{fs::File, io::Read};
pub fn comm(pid: u32) -> (String, &'static str) {
    let result = File::open(format!("/proc/{pid}/comm")).and_then(|f| {
        let mut bytes = Vec::with_capacity(64);
        f.take(64).read_to_end(&mut bytes)?;
        Ok(bytes)
    });
    match result {
        Ok(bytes) => match std::str::from_utf8(&bytes) {
            Ok(text) => {
                let name = sanitize(text);
                if name.is_empty() {
                    ("unknown".into(), "invalid")
                } else {
                    (name, "best_effort")
                }
            }
            Err(_) => ("unknown".into(), "invalid"),
        },
        Err(_) => ("unknown".into(), "unavailable"),
    }
}
fn sanitize(value: &str) -> String {
    value
        .chars()
        .filter(|c| !c.is_control())
        .take(32)
        .map(|c| if c.is_whitespace() { '_' } else { c })
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_process_and_controls() {
        assert_eq!(comm(u32::MAX), ("unknown".into(), "unavailable"));
        assert_eq!(sanitize("a\x1b\n b"), "a_b");
        assert_eq!(sanitize(&"x".repeat(100)).len(), 32);
    }
}
