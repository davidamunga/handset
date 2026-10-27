//! Finding phones on the network without asking adb. `adb mdns services` is often empty (or misses the short
//! pairing window) while the system's own mDNS sees the phone straight away, so we browse for the two adb service
//! types ourselves and merge the result with adb's list.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use mdns_sd::{ServiceDaemon, ServiceEvent};

use crate::adb::{Service, ServiceKind};

const CONNECT: &str = "_adb-tls-connect._tcp.local.";
const PAIRING: &str = "_adb-tls-pairing._tcp.local.";

/// Services seen so far, by full instance name, so a removal finds what it removes.
static SEEN: Mutex<Option<HashMap<String, Service>>> = Mutex::new(None);
static STARTED: OnceLock<bool> = OnceLock::new();

/// Begin browsing, once. If the daemon cannot start (no usable network interface) we just rely on adb.
pub fn start() {
    STARTED.get_or_init(|| {
        let Ok(daemon) = ServiceDaemon::new() else { return false };
        for ty in [CONNECT, PAIRING] {
            let Ok(events) = daemon.browse(ty) else { continue };
            std::thread::spawn(move || {
                while let Ok(event) = events.recv() {
                    apply(event);
                }
            });
        }
        // the daemon stops with the process; keep it alive until then
        std::mem::forget(daemon);
        true
    });
}

/// What the browser has found so far.
pub fn services() -> Vec<Service> {
    let mut list: Vec<Service> =
        SEEN.lock().ok().and_then(|s| s.as_ref().map(|m| m.values().cloned().collect())).unwrap_or_default();
    list.sort_by(|a, b| a.name.cmp(&b.name));
    list
}

fn apply(event: ServiceEvent) {
    let Ok(mut seen) = SEEN.lock() else { return };
    let seen = seen.get_or_insert_with(HashMap::new);
    match event {
        ServiceEvent::ServiceResolved(info) => {
            let mut ips: Vec<_> = info.get_addresses_v4().into_iter().collect();
            ips.sort();
            if let (Some(ip), Some((name, kind))) = (ips.first(), split_fullname(info.get_fullname())) {
                let addr = format!("{ip}:{}", info.get_port());
                seen.insert(info.get_fullname().to_string(), Service { name, kind, addr });
            }
        }
        ServiceEvent::ServiceRemoved(_, fullname) => {
            seen.remove(&fullname);
        }
        _ => {}
    }
}

/// `handset-abc123._adb-tls-pairing._tcp.local.` as the instance name and which adb service it is.
fn split_fullname(fullname: &str) -> Option<(String, ServiceKind)> {
    let (name, ty) = fullname.split_once("._adb-tls-")?;
    let kind = if ty.starts_with("pairing.") {
        ServiceKind::Pairing
    } else if ty.starts_with("connect.") {
        ServiceKind::Connect
    } else {
        return None;
    };
    Some((name.to_string(), kind))
}

/// Everything adb and our own browser found, each service once (the same phone shows up in both).
pub fn merge(adb: Vec<Service>, ours: Vec<Service>) -> Vec<Service> {
    let mut all = adb;
    for s in ours {
        if !all.iter().any(|a| a.kind == s.kind && (a.addr == s.addr || a.name == s.name)) {
            all.push(s);
        }
    }
    all
}

#[cfg(test)]
mod tests {
    use super::*;

    fn svc(name: &str, kind: ServiceKind, addr: &str) -> Service {
        Service { name: name.into(), kind, addr: addr.into() }
    }

    #[test]
    fn instance_names_are_split_from_the_service_type() {
        assert_eq!(
            split_fullname("handset-xrndqy._adb-tls-pairing._tcp.local."),
            Some(("handset-xrndqy".into(), ServiceKind::Pairing))
        );
        assert_eq!(
            split_fullname("adb-PT19655JA1222400122-zuKyEQ._adb-tls-connect._tcp.local."),
            Some(("adb-PT19655JA1222400122-zuKyEQ".into(), ServiceKind::Connect))
        );
        assert_eq!(split_fullname("printer._ipp._tcp.local."), None);
    }

    #[test]
    fn the_same_phone_found_twice_is_listed_once() {
        let a = vec![svc("adb-1", ServiceKind::Connect, "192.168.1.9:40000")];
        let b = vec![
            svc("adb-1", ServiceKind::Connect, "192.168.1.9:40000"),
            svc("handset-x", ServiceKind::Pairing, "192.168.1.9:41000"),
        ];
        let m = merge(a, b);
        assert_eq!(m.len(), 2);
        assert!(m.iter().any(|s| s.kind == ServiceKind::Pairing && s.name == "handset-x"));
    }

    #[test]
    fn a_pairing_and_a_connect_service_with_one_name_are_both_kept() {
        let m =
            merge(vec![svc("a", ServiceKind::Connect, "1.1.1.1:1")], vec![svc("a", ServiceKind::Pairing, "1.1.1.1:2")]);
        assert_eq!(m.len(), 2);
    }
}
