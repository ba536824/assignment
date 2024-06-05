use crate::drbd::{
    BackingDevice, Connection, ConnectionState, Device, DiskState, EventType, EventUpdate, Path,
    PeerDevice, ReplicationState, Resource, Role,
};
use anyhow::{Context, Result};
use crossbeam_channel::{SendError, Sender};
use log::{debug, warn};
use std::io::BufRead;
use std::io::BufReader;
use std::io::Write;
use std::process::{Command, Stdio};
use std::str::FromStr;
use std::thread;
use std::time::Duration;

pub fn events2(tx: Sender<EventUpdate>, statistics_poll: Duration) -> Result<()> {
    let mut send_flush = false;
    loop {
        debug!("events2_loop: starting process_events2 loop");
        let result = process_events2(&tx, statistics_poll, send_flush);
        send_flush = true;
        match result {
            Ok(()) => break,
            Err(e) => {
                if e.is::<SendError<EventUpdate>>() {
                    debug!("events2_loop: send error on chanel, bye");
                    return Err(e);
                }
                thread::sleep(Duration::from_secs(2));
            }
        }
    }

    Ok(())
}

struct KillOnDrop(std::process::Child);
impl Drop for KillOnDrop {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn process_events2(
    tx: &Sender<EventUpdate>,
    statistics_poll: Duration,
    send_flush: bool,
) -> Result<()> {
    let mut cmd = Command::new("drbdsetup")
        .arg("events2")
        .arg("--full")
        .arg("--poll")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .with_context(|| {
            "events: process_events2: could not spawn 'drbdsetup events2 --full --poll'"
        })?;

    let mut stdin = cmd
        .stdin
        .take()
        .expect("events:: process_events2: stdin set to Stdio::piped()");
    thread::spawn(move || loop {
        if let Err(e) = stdin.write_all("n\n".as_bytes()) {
            warn!("process_events2: could not update statistics: {}", e);
            break;
        }
        thread::sleep(statistics_poll);
    });

    let stdout = cmd
        .stdout
        .take()
        .expect("events: process_events2: stdout set to Stdio::piped()");

    let _kill_cmd = KillOnDrop(cmd);

    // great, we established a successful events2 tracking
    if send_flush {
        debug!("process_events2: sending flush");
        tx.send(EventUpdate::Flush)?;
    }

    let mut reader = BufReader::new(stdout);

    let mut buf = String::new();
    while reader.read_line(&mut buf)? != 0 {
        // be careful here, every continue needs a buf.clear()!
        let line = buf.trim();
        if line == "exists -" {
            buf.clear();
            continue;
        }

        match parse_events2_line(line) {
            Ok(update) => tx.send(update)?,
            Err(e) => debug!(
                "process_events2: could not parse line '{}', because {}",
                line, e
            ),
        }
        buf.clear();
    }

    warn!("process_events2: exit");
    Err(anyhow::anyhow!("events: process_events2: exit"))
}

fn parse_events2_line(line: &str) -> Result<EventUpdate> {
    let mut words = line.split_whitespace();

    let verb = words.next().unwrap_or_default();
    let et = match EventType::from_str(verb) {
        Ok(et) => et,
        Err(_) => {
            return Err(anyhow::anyhow!(
                "events: parse_events2_line: unknown events type: {}",
                verb
            ));
        }
    };

    let what = words.next().unwrap_or_default();
    let kvs = words.filter_map(parse_kv);
    if what == "resource" {
        let mut resource = Resource {
            ..Default::default()
        };

        for (k, v) in kvs {
            match (k, v) {
                ("name", v) => resource.name = v.into(),
                ("role", v) => resource.role = Role::from_str(v)?,
                ("suspended", v) => resource.suspended = str_to_bool(v),
                ("write-ordering", v) => resource.write_ordering = v.to_string(),
                ("force-io-failures", v) => resource.force_io_failures = str_to_bool(v),
                ("may_promote", v) => resource.may_promote = str_to_bool(v),
                ("promotion_score", v) => resource.promotion_score = v.parse::<_>()?,
                _ => {
                    warn!("events: process_events2: resource: unknown keyword '{}'", k)
                }
            };
        }
        return Ok(EventUpdate::Resource(et, resource));
    } else if what == "device" {
        let mut device = Device {
            ..Default::default()
        };
        for (k, v) in kvs {
            match (k, v) {
                ("name", v) => device.name = v.into(),
                ("volume", v) => device.volume = v.parse::<_>()?,
                ("minor", v) => device.minor = v.parse::<_>()?,
                ("disk", v) => device.disk_state = DiskState::from_str(v)?,
                ("client", v) => device.client = str_to_bool(v),
                ("backing_dev", v) => device.backing_dev = BackingDevice::from_str(v)?,
                ("quorum", v) => device.quorum = str_to_bool(v),
                ("size", v) => device.size = v.parse::<_>()?,
                ("read", v) => device.read = v.parse::<_>()?,
                ("written", v) => device.written = v.parse::<_>()?,
                ("al-writes", v) => device.al_writes = v.parse::<_>()?,
                ("bm-writes", v) => device.bm_writes = v.parse::<_>()?,
                ("upper-pending", v) => device.upper_pending = v.parse::<_>()?,
                ("lower-pending", v) => device.lower_pending = v.parse::<_>()?,
                ("al-suspended", v) => device.al_suspended = str_to_bool(v),
                ("blocked", v) => device.blocked = v.into(),
                _ => {
                    warn!("events: process_events2: device: unknown keyword '{}'", k)
                }
            };
        }
        return Ok(EventUpdate::Device(et, device));
    } else if what == "connection" {
        let mut conn = Connection {
            ..Default::default()
        };
        for (k, v) in kvs {
            match (k, v) {
                ("name", v) => conn.name = v.into(),
                ("peer-node-id", v) => conn.peer_node_id = v.parse::<_>()?,
                ("conn-name", v) => conn.conn_name = v.to_string(),
                ("connection", v) => conn.connection = ConnectionState::from_str(v)?,
                ("role", v) => conn.peer_role = Role::from_str(v)?,
                ("congested", v) => conn.congested = str_to_bool(v),
                ("ap-in-flight", v) => conn.ap_in_flight = v.parse::<_>()?,
                ("rs-in-flight", v) => conn.rs_in_flight = v.parse::<_>()?,
                _ => {
                    warn!(
                        "events: process_events2: connection: unknown keyword '{}'",
                        k
                    )
                }
            };
        }
        return Ok(EventUpdate::Connection(et, conn));
    } else if what == "peer-device" {
        let mut peerdevice = PeerDevice {
            has_sync_details: false,
            has_online_verify_details: false,
            ..Default::default()
        };
        for (k, v) in kvs {
            match (k, v) {
                ("name", v) => peerdevice.name = v.into(),
                ("conn-name", v) => peerdevice.conn_name = v.into(),
                ("volume", v) => peerdevice.volume = v.parse::<_>()?,
                ("peer-node-id", v) => peerdevice.peer_node_id = v.parse::<_>()?,
                ("replication", v) => peerdevice.replication_state = ReplicationState::from_str(v)?,
                ("peer-disk", v) => peerdevice.peer_disk_state = DiskState::from_str(v)?,
                ("peer-client", v) => peerdevice.peer_client = str_to_bool(v),
                ("resync-suspended", v) => peerdevice.resync_suspended = str_to_bool(v),
                ("received", v) => peerdevice.received = v.parse::<_>()?,
                ("sent", v) => peerdevice.sent = v.parse::<_>()?,
                ("out-of-sync", v) => peerdevice.out_of_sync = v.parse::<_>()?,
                ("pending", v) => peerdevice.pending = v.parse::<_>()?,
                ("unacked", v) => peerdevice.unacked = v.parse::<_>()?,
                ("done", _) => (),
                ("eta", _) => (),
                ("dbdt1", _) => (),
                _ => {
                    warn!(
                        "events: process_events2: peer-device: unknown keyword '{}'",
                        k
                    )
                }
            };
        }
        return Ok(EventUpdate::PeerDevice(et, peerdevice));
    } else if what == "path" {
        let mut path = Path {
            ..Default::default()
        };
        for (k, v) in kvs {
            match (k, v) {
                ("name", v) => path.name = v.into(),
                ("peer-node-id", v) => path.peer_node_id = v.parse::<_>()?,
                ("conn-name", v) => path.conn_name = v.into(),
                ("local", v) => path.local = v.into(),
                ("peer", v) => path.peer = v.into(),
                ("established", v) => path.established = str_to_bool(v),
                _ => {
                    warn!("events: process_events2: path: unknown keyword '{}'", k)
                }
            }
        }
        return Ok(EventUpdate::Path(et, path));
    }

    Err(anyhow::anyhow!(
        "events: process_events2: unknown keyword '{}'",
        what
    ))
}

fn parse_kv(item: &str) -> Option<(&str, &str)> {
    let mut iter = item.splitn(2, ':');
    match (iter.next(), iter.next()) {
        (Some(k), Some(v)) => Some((k, v)),
        _ => None,
    }
}

// this implements a logic that is appropriate for events2 "bools"
// usually one would be a bit more conservative, but we want to map things like "suspended:user" to "true".
fn str_to_bool(s: &str) -> bool {
    !(s == "no" || s == "false")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_to_bool() {
        assert!(str_to_bool("yes"));
        assert!(!str_to_bool("no"));
        assert!(str_to_bool("true"));
        assert!(!str_to_bool("false"));
        assert!(str_to_bool("user"));
    }

    #[test]
    fn all_parsed_resource_update() {
        let up = parse_events2_line("exists resource name:foo role:Primary suspended:yes write-ordering:foo may_promote:yes promotion_score:23").unwrap();
        let expected = EventUpdate::Resource(
            EventType::Exists,
            Resource {
                name: "foo".to_string(),
                role: Role::Primary,
                suspended: true,
                write_ordering: "foo".to_string(),
                may_promote: true,
                promotion_score: 23,
                force_io_failures: false,
                devices: vec![],
                connections: vec![],
            },
        );
        assert_eq!(up, expected);
        let up = parse_events2_line("exists resource xxx:bla name:foo role:Primary suspended:yes write-ordering:foo may_promote:yes promotion_score:23").unwrap();
        assert_eq!(up, expected);
    }

    #[test]
    fn all_parsed_device_update() {
        let up = parse_events2_line("change device name:foo volume:1 minor:1 disk:Attaching backing_dev:/dev/sda1 client:no quorum:yes size:1 read:1 written:1 al-writes:1 bm-writes:1 upper-pending:1 lower-pending:1 al-suspended:yes blocked:upper").unwrap();
        let expected = EventUpdate::Device(
            EventType::Change,
            Device {
                name: "foo".to_string(),
                volume: 1,
                minor: 1,
                disk_state: DiskState::Attaching,
                client: false,
                backing_dev: BackingDevice(Some("/dev/sda1".to_string())),
                quorum: true,
                size: 1,
                read: 1,
                written: 1,
                al_writes: 1,
                bm_writes: 1,
                upper_pending: 1,
                lower_pending: 1,
                al_suspended: true,
                blocked: "upper".to_string(),
            },
        );
        assert_eq!(up, expected);
        let up = parse_events2_line("change device name:foo xxx:bla volume:1 minor:1 disk:Attaching backing_dev:/dev/sda1 client:no quorum:yes size:1 read:1 written:1 al-writes:1 bm-writes:1 upper-pending:1 lower-pending:1 al-suspended:yes blocked:upper").unwrap();
        assert_eq!(up, expected);

        // backing_dev as none
        let up = parse_events2_line("change device name:foo volume:1 minor:1 disk:Attaching backing_dev:none client:yes quorum:yes size:1 read:1 written:1 al-writes:1 bm-writes:1 upper-pending:1 lower-pending:1 al-suspended:yes blocked:no").unwrap();
        let expected = EventUpdate::Device(
            EventType::Change,
            Device {
                name: "foo".to_string(),
                volume: 1,
                minor: 1,
                disk_state: DiskState::Attaching,
                client: true,
                backing_dev: BackingDevice(None),
                quorum: true,
                size: 1,
                read: 1,
                written: 1,
                al_writes: 1,
                bm_writes: 1,
                upper_pending: 1,
                lower_pending: 1,
                al_suspended: true,
                blocked: "no".to_string(),
            },
        );
        assert_eq!(up, expected);
    }

    #[test]
    fn all_parsed_connection_update() {
        let up = parse_events2_line("exists connection name:foo peer-node-id:1 conn-name:bar connection:Connected role:Primary congested:yes ap-in-flight:1 rs-in-flight:1").unwrap();
        let expected = EventUpdate::Connection(
            EventType::Exists,
            Connection {
                name: "foo".to_string(),
                peer_node_id: 1,
                conn_name: "bar".to_string(),
                connection: ConnectionState::Connected,
                peer_role: Role::Primary,
                congested: true,
                ap_in_flight: 1,
                rs_in_flight: 1,
                peerdevices: vec![],
                paths: vec![],
            },
        );
        assert_eq!(up, expected);
        let up = parse_events2_line("exists connection name:foo xxx:bla peer-node-id:1 conn-name:bar connection:Connected role:Primary congested:yes ap-in-flight:1 rs-in-flight:1").unwrap();
        assert_eq!(up, expected);
    }

    #[test]
    fn all_parsed_peerdevice_update() {
        let up = parse_events2_line("exists peer-device name:foo peer-node-id:1 conn-name:bar volume:1 replication:Established peer-disk:UpToDate peer-client:yes resync-suspended:yes received:1 sent:1 out-of-sync:1 pending:1 unacked:1").unwrap();
        let expected = EventUpdate::PeerDevice(
            EventType::Exists,
            PeerDevice {
                name: "foo".to_string(),
                peer_node_id: 1,
                conn_name: "bar".to_string(),
                volume: 1,
                replication_state: ReplicationState::Established,
                peer_disk_state: DiskState::UpToDate,
                peer_client: true,
                resync_suspended: true,
                received: 1,
                sent: 1,
                out_of_sync: 1,
                pending: 1,
                unacked: 1,
                has_sync_details: false,
                has_online_verify_details: false,
            },
        );
        assert_eq!(up, expected);
        let up = parse_events2_line("exists peer-device name:foo xxx:bla peer-node-id:1 conn-name:bar volume:1 replication:Established peer-disk:UpToDate peer-client:yes resync-suspended:yes received:1 sent:1 out-of-sync:1 pending:1 unacked:1").unwrap();
        assert_eq!(up, expected);
    }

    #[test]
    fn all_parsed_path_update() {
        let up = parse_events2_line("change path name:foo peer-node-id:3 conn-name:bar local:ipv4:1.2.3.4:7020 peer:ipv4:1.2.3.5:7020 established:yes").unwrap();
        let expected = EventUpdate::Path(
            EventType::Change,
            Path {
                name: "foo".to_string(),
                peer_node_id: 3,
                conn_name: "bar".to_string(),
                local: "ipv4:1.2.3.4:7020".to_string(),
                peer: "ipv4:1.2.3.5:7020".to_string(),
                established: true,
            },
        );
        assert_eq!(up, expected);
    }

    #[test]
    fn wrong_et() {
        assert!(parse_events2_line("xxx resource name:foo").is_err());
        // these will be implemented soon, but for now they are errors
        assert!(parse_events2_line("call helper").is_err());
        assert!(parse_events2_line("response helper").is_err());
    }

    #[test]
    fn wrong_what() {
        assert!(parse_events2_line("exists xxx name:foo").is_err());
    }
}
