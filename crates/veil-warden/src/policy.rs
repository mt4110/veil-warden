// SPDX-License-Identifier: MIT
use std::net::IpAddr;
use veil_warden_common::{RuleKey, TCP, rule_key};
pub fn key(ip: &str, port: &str) -> Result<RuleKey, Box<dyn std::error::Error>> {
    let ip: IpAddr = ip.parse()?;
    let port: u16 = port.parse()?;
    if port == 0 {
        return Err("port must be 1..65535".into());
    }
    let mut address = [0; 16];
    let family = match ip {
        IpAddr::V4(ip) => {
            address[..4].copy_from_slice(&ip.octets());
            4
        }
        IpAddr::V6(ip) => {
            address = ip.octets();
            6
        }
    };
    Ok(rule_key(address, family, port, TCP))
}
pub fn destination(k: &RuleKey) -> String {
    let port = u16::from_le_bytes([k[16], k[17]]);
    if k[18] == 4 {
        format!("{}:{port}", std::net::Ipv4Addr::new(k[0], k[1], k[2], k[3]))
    } else {
        format!(
            "[{}]:{port}",
            std::net::Ipv6Addr::from(<[u8; 16]>::try_from(&k[..16]).unwrap())
        )
    }
}
#[cfg(target_os = "linux")]
pub mod control {
    use super::*;
    use aya::maps::{HashMap, MapData};
    use std::{
        os::linux::net::SocketAddrExt,
        os::unix::net::{SocketAddr, UnixListener, UnixStream},
        time::Duration,
    };
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use veil_warden_common::POLICY_CAPACITY;
    const SOCKET: &[u8] = b"veil-warden-policy";
    type Error = Box<dyn std::error::Error>;
    pub struct Controller {
        pub listener: tokio::net::UnixListener,
        map: HashMap<MapData, RuleKey, u32>,
        enforce: bool,
        next_id: u32,
    }
    impl Controller {
        pub fn new(
            map: HashMap<MapData, RuleKey, u32>,
            enforce: bool,
            initial: Option<RuleKey>,
        ) -> Result<Self, Error> {
            let listener = UnixListener::bind_addr(&SocketAddr::from_abstract_name(SOCKET)?)?;
            listener.set_nonblocking(true)?;
            let mut this = Self {
                listener: tokio::net::UnixListener::from_std(listener)?,
                map,
                enforce,
                next_id: 1,
            };
            if let Some(k) = initial {
                this.add(k)?;
            }
            Ok(this)
        }
        pub(crate) fn add(&mut self, k: RuleKey) -> Result<u32, Error> {
            if !self.enforce {
                return Err("observe mode cannot change deny rules".into());
            }
            if k[19] != TCP
                || !matches!(k[18], 4 | 6)
                || k[16..18] == [0, 0]
                || (k[18] == 4 && k[4..16] != [0; 12])
            {
                return Err("invalid TCP destination tuple".into());
            }
            let id = self.next_id;
            let next = id.checked_add(1).ok_or("policy ID exhausted")?;
            // BPF_NOEXIST: duplicate keys and capacity errors leave the map unchanged.
            self.map.insert(k, id, 1)?;
            self.next_id = next;
            Ok(id)
        }
        pub(crate) fn rules(&self) -> Result<Vec<(RuleKey, u32)>, Error> {
            let mut rules = self.map.iter().collect::<Result<Vec<_>, _>>()?;
            rules.sort_by_key(|(key, _)| *key);
            Ok(rules)
        }
        pub(crate) fn remove(&mut self, key: RuleKey) -> Result<(), Error> {
            if !self.enforce {
                return Err("observe mode cannot change deny rules".into());
            }
            self.map.remove(&key)?;
            Ok(())
        }
        fn list(&self) -> Result<String, Error> {
            let mut rows = Vec::new();
            for item in self.map.iter() {
                let (k, id) = item?;
                rows.push(format!(
                    "rule id={id} destination={} protocol=tcp",
                    destination(&k)
                ));
            }
            rows.sort();
            Ok(format!(
                "mode={} rules={} capacity={}\n{}",
                if self.enforce { "enforce" } else { "observe" },
                rows.len(),
                POLICY_CAPACITY,
                rows.join("\n")
            ))
        }
        fn command(&mut self, request: &str) -> Result<String, Error> {
            let parts: Vec<_> = request.split_whitespace().collect();
            match parts.as_slice() {
                ["list"] => Ok(self.list()?),
                ["add", ip, port] => {
                    let k = key(ip, port)?;
                    let id = self.add(k)?;
                    Ok(format!(
                        "added id={id} destination={}\n{}",
                        destination(&k),
                        self.list()?
                    ))
                }
                ["remove", ip, port] => {
                    if !self.enforce {
                        return Err("observe mode cannot change deny rules".into());
                    }
                    let k = key(ip, port)?;
                    self.remove(k)?;
                    Ok(format!(
                        "removed destination={}\n{}",
                        destination(&k),
                        self.list()?
                    ))
                }
                _ => Err("expected list | add IP PORT | remove IP PORT".into()),
            }
        }
        pub async fn serve(&mut self, mut stream: tokio::net::UnixStream) -> Result<(), Error> {
            if stream.peer_cred()?.uid() != 0 {
                return Ok(());
            }
            // The request ends at EOF. Bound allocation and time even for local root.
            let exchange = async {
                let mut bytes = Vec::new();
                (&mut stream).take(257).read_to_end(&mut bytes).await?;
                let result = if bytes.len() > 256 {
                    Err("request exceeds 256 bytes".into())
                } else {
                    std::str::from_utf8(&bytes)
                        .map_err(|e| -> Error { e.into() })
                        .and_then(|s| self.command(s))
                };
                let reply = match result {
                    Ok(s) => format!("ok {s}\n"),
                    Err(e) => {
                        let mut detail = e.to_string();
                        let mut source = e.source();
                        while let Some(cause) = source {
                            detail.push_str(&format!(": {cause}"));
                            source = cause.source();
                        }
                        format!("error {detail}\n{}\n", self.list()?)
                    }
                };
                stream.write_all(reply.as_bytes()).await?;
                Ok::<(), Error>(())
            };
            // A disconnected/malformed control client cannot tear down the monitor.
            if let Ok(Err(error)) = tokio::time::timeout(Duration::from_secs(2), exchange).await {
                eprintln!("policy control transport error: {error}");
            }
            Ok(())
        }
    }
    pub fn client(args: &[String]) -> Result<(), Error> {
        if std::fs::read_to_string("/etc/hostname")?.trim() != "veil-warden-sandbox" {
            return Err("policy control requires dedicated VM".into());
        }
        match args {
            [op] if op == "list" => {}
            [op, ip, port] if op == "add" || op == "remove" => {
                key(ip, port)?;
            }
            _ => return Err("policy list | add IP PORT | remove IP PORT".into()),
        }
        let stream = UnixStream::connect_addr(&SocketAddr::from_abstract_name(SOCKET)?)?;
        stream.set_nonblocking(true)?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .enable_time()
            .build()?;
        let reply = runtime.block_on(async {
            let mut stream = tokio::net::UnixStream::from_std(stream)?;
            if stream.peer_cred()?.uid() != 0 {
                return Err::<String, Error>("policy server must be root".into());
            }
            tokio::time::timeout(Duration::from_secs(3), async {
                stream.write_all(args.join(" ").as_bytes()).await?;
                stream.shutdown().await?;
                let mut reply = String::new();
                stream.take(4097).read_to_string(&mut reply).await?;
                if reply.len() > 4096 {
                    return Err("control response exceeds limit".into());
                }
                Ok::<String, Error>(reply)
            })
            .await?
        })?;
        print!("{reply}");
        if !reply.starts_with("ok ") {
            return Err("policy update rejected; see current state above".into());
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strict_tuple() {
        for (ip, port) in [
            ("bad", "443"),
            ("localhost", "443"),
            ("::1", "0"),
            ("127.0.0.1", "65536"),
        ] {
            assert!(key(ip, port).is_err());
        }
        assert_eq!(
            key("127.0.0.1", "443").unwrap(),
            key("::ffff:127.0.0.1", "443").unwrap()
        );
        assert_ne!(key("::1", "443").unwrap(), key("127.0.0.1", "443").unwrap());
        assert_eq!(destination(&key("::1", "443").unwrap()), "[::1]:443");
    }
}
