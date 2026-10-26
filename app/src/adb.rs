//! Thin wrapper around the `adb` executable.

use std::io;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn adb_path() -> PathBuf {
    crate::tools::find("adb")
}

pub fn command(serial: &str) -> Command {
    let mut c = Command::new(adb_path());
    c.args(["-s", serial]);
    c
}

pub fn run(serial: &str, args: &[&str]) -> io::Result<String> {
    let out = command(serial).args(args).stdin(Stdio::null()).output()?;
    if !out.status.success() {
        return Err(io::Error::other(format!(
            "adb {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Every device adb lists, with its state (`device`, `unauthorized`, `offline`, ...).
pub fn list() -> io::Result<Vec<(String, String)>> {
    let out = Command::new(adb_path()).arg("devices").output()?;
    Ok(parse_devices(&String::from_utf8_lossy(&out.stdout)))
}

/// Serials of devices in the `device` state.
pub fn devices() -> io::Result<Vec<String>> {
    Ok(list()?.into_iter().filter(|(_, state)| state == "device").map(|(serial, _)| serial).collect())
}

fn parse_devices(text: &str) -> Vec<(String, String)> {
    text.lines()
        .skip(1)
        .filter_map(|l| {
            let mut it = l.split_whitespace();
            Some((it.next()?.to_string(), it.next()?.to_string()))
        })
        .collect()
}

/// A phone (or its pairing window) that announced itself on the network through mDNS.
#[derive(Debug, Clone, PartialEq)]
pub struct Service {
    pub name: String,
    pub kind: ServiceKind,
    /// `ip:port`
    pub addr: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ServiceKind {
    /// `_adb-tls-connect`: a phone with Wireless debugging on that can be connected to.
    Connect,
    /// `_adb-tls-pairing`: a phone showing its pairing screen or scanning a pairing QR code.
    Pairing,
}

/// `adb mdns services`: the phones announcing themselves on this network.
pub fn mdns_services() -> io::Result<Vec<Service>> {
    let mut cmd = Command::new(adb_path());
    cmd.args(["mdns", "services"]);
    let out = output_within(cmd, std::time::Duration::from_secs(6))?;
    Ok(parse_mdns(&String::from_utf8_lossy(&out.stdout)))
}

fn parse_mdns(text: &str) -> Vec<Service> {
    text.lines()
        .filter_map(|l| {
            let mut it = l.split_whitespace();
            let (name, ty, addr) = (it.next()?, it.next()?, it.next()?);
            let kind = if ty.starts_with("_adb-tls-connect") {
                ServiceKind::Connect
            } else if ty.starts_with("_adb-tls-pairing") {
                ServiceKind::Pairing
            } else {
                return None;
            };
            addr.contains(':').then(|| Service { name: name.to_string(), kind, addr: addr.to_string() })
        })
        .collect()
}

/// `host` or `host:port` as `host:port`, with adb's default port when none is given.
pub fn normalize_addr(addr: &str) -> String {
    if addr.contains(':') { addr.to_string() } else { format!("{addr}:5555") }
}

/// Run `cmd` and collect its output, giving up (and killing it) after `limit`. adb's own connect
/// timeout is over a minute, which looks like a hang.
fn output_within(mut cmd: Command, limit: std::time::Duration) -> io::Result<std::process::Output> {
    let mut child = cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
    let deadline = std::time::Instant::now() + limit;
    loop {
        if child.try_wait()?.is_some() {
            return child.wait_with_output();
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::new(io::ErrorKind::TimedOut, "no answer in time"));
        }
        std::thread::sleep(std::time::Duration::from_millis(40));
    }
}

const CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// `adb connect`: returns the serial to use afterwards.
pub fn connect(addr: &str) -> io::Result<String> {
    let addr = normalize_addr(addr);
    let mut cmd = Command::new(adb_path());
    cmd.args(["connect", &addr]);
    match output_within(cmd, CONNECT_TIMEOUT) {
        Ok(out) => parse_connect(&String::from_utf8_lossy(&out.stdout), &String::from_utf8_lossy(&out.stderr), &addr),
        Err(e) if e.kind() == io::ErrorKind::TimedOut => {
            parse_connect("", &format!("no answer from {addr} after {} seconds", CONNECT_TIMEOUT.as_secs()), &addr)
        }
        Err(e) => Err(e),
    }
}

fn parse_connect(stdout: &str, stderr: &str, addr: &str) -> io::Result<String> {
    let text = format!("{} {}", stdout.trim(), stderr.trim());
    let lower = text.to_lowercase();
    if lower.contains("connected to") && !lower.contains("failed") && !lower.contains("cannot") {
        return Ok(addr.to_string());
    }
    let mut reason = text.trim().to_string();
    if reason.is_empty() {
        reason = "no answer from adb".to_string();
    }
    Err(io::Error::other(format!(
        "Could not connect to {addr}: {reason}\n\nCheck that the phone and this Mac are on the same Wi-Fi, and that Wireless \
         debugging is on (Developer options). On Android 11 and later, pair first with `handset --pair <ip:port> <code>`."
    )))
}

/// `adb pair host:port code`, as shown under Developer options > Wireless debugging > Pair device.
pub fn pair(addr: &str, code: &str) -> io::Result<String> {
    let mut cmd = Command::new(adb_path());
    cmd.args(["pair", addr, code]);
    let out = output_within(cmd, std::time::Duration::from_secs(20)).map_err(|e| {
        if e.kind() == io::ErrorKind::TimedOut {
            io::Error::other(format!("Pairing with {addr} timed out: no answer after 20 seconds. Check the address and that the pairing screen is still open."))
        } else {
            e
        }
    })?;
    let text =
        format!("{} {}", String::from_utf8_lossy(&out.stdout).trim(), String::from_utf8_lossy(&out.stderr).trim());
    if text.to_lowercase().contains("successfully paired") {
        Ok(text.trim().to_string())
    } else {
        Err(io::Error::other(format!(
            "Pairing with {addr} failed: {}\n\nUse the address and the six digit code from the \"Pair device with pairing code\" \
             screen (the pairing port differs from the connection port), and finish within the time shown.",
            text.trim()
        )))
    }
}

/// Switch a USB-connected phone to adb over Wi-Fi and connect to it. Returns the new serial.
pub fn wifi(usb_serial: &str) -> io::Result<String> {
    let route = run(usb_serial, &["shell", "ip", "route", "get", "1.1.1.1"]).unwrap_or_default();
    let ip = parse_route_src(&route)
        .or_else(|| {
            run(usb_serial, &["shell", "ip", "-f", "inet", "addr", "show", "wlan0"]).ok().and_then(|t| parse_inet(&t))
        })
        .ok_or_else(|| io::Error::other("Could not find the phone's Wi-Fi address. Is it connected to Wi-Fi?"))?;
    run(usb_serial, &["tcpip", "5555"])?;
    // adb restarts on the phone; give it a moment, then retry the connection a few times
    let addr = format!("{ip}:5555");
    let mut last = None;
    for _ in 0..6 {
        std::thread::sleep(std::time::Duration::from_millis(800));
        match connect(&addr) {
            Ok(s) => return Ok(s),
            Err(e) => last = Some(e),
        }
    }
    Err(last.unwrap_or_else(|| io::Error::other("could not connect")))
}

/// `ip route get 1.1.1.1` output: `... src 192.168.1.23 ...`.
fn parse_route_src(text: &str) -> Option<String> {
    let mut it = text.split_whitespace();
    while let Some(w) = it.next() {
        if w == "src" {
            return it.next().filter(|ip| ip.parse::<std::net::Ipv4Addr>().is_ok()).map(str::to_string);
        }
    }
    None
}

/// `ip -f inet addr show wlan0` output: `inet 192.168.1.23/24 ...`.
fn parse_inet(text: &str) -> Option<String> {
    let mut it = text.split_whitespace();
    while let Some(w) = it.next() {
        if w == "inet" {
            return it
                .next()
                .and_then(|a| a.split('/').next())
                .filter(|ip| ip.parse::<std::net::Ipv4Addr>().is_ok())
                .map(str::to_string);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIST: &str =
        "List of devices attached\nW82202003817\tdevice\n192.168.1.5:5555\toffline\nR58M\tunauthorized\n\n";

    #[test]
    fn parses_device_states() {
        assert_eq!(
            parse_devices(LIST),
            vec![
                ("W82202003817".into(), "device".into()),
                ("192.168.1.5:5555".into(), "offline".into()),
                ("R58M".into(), "unauthorized".into())
            ]
        );
        assert!(parse_devices("List of devices attached\n\n").is_empty());
    }

    #[test]
    fn gives_up_on_a_slow_command() {
        let mut slow = Command::new("sleep");
        slow.arg("5");
        let started = std::time::Instant::now();
        let err = output_within(slow, std::time::Duration::from_millis(200)).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::TimedOut);
        assert!(started.elapsed() < std::time::Duration::from_secs(2));

        let mut fast = Command::new("echo");
        fast.arg("hi");
        let out = output_within(fast, std::time::Duration::from_secs(5)).unwrap();
        assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hi");
    }

    #[test]
    fn reads_mdns_services() {
        let text = "List of discovered mdns services\n\
                    adb-W82202003817-MPn68n\t_adb-tls-connect._tcp.\t192.168.0.20:41789\n\
                    handset-ab12cd\t_adb-tls-pairing._tcp\t192.168.0.20:37001\n\
                    printer\t_ipp._tcp\t192.168.0.9:631\n\n";
        let s = parse_mdns(text);
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].kind, ServiceKind::Connect);
        assert_eq!(s[0].addr, "192.168.0.20:41789");
        assert_eq!(
            s[1],
            Service { name: "handset-ab12cd".into(), kind: ServiceKind::Pairing, addr: "192.168.0.20:37001".into() }
        );
        assert!(parse_mdns("List of discovered mdns services\n\n").is_empty());
    }

    #[test]
    fn adds_the_default_port() {
        assert_eq!(normalize_addr("192.168.1.5"), "192.168.1.5:5555");
        assert_eq!(normalize_addr("192.168.1.5:37123"), "192.168.1.5:37123");
    }

    #[test]
    fn reads_connect_results() {
        assert_eq!(parse_connect("connected to 10.0.0.2:5555\n", "", "10.0.0.2:5555").unwrap(), "10.0.0.2:5555");
        assert!(parse_connect("already connected to 10.0.0.2:5555", "", "10.0.0.2:5555").is_ok());
        let err =
            parse_connect("failed to connect to '10.0.0.2:5555': Connection refused", "", "10.0.0.2:5555").unwrap_err();
        assert!(err.to_string().contains("same Wi-Fi"), "{err}");
        assert!(parse_connect("", "", "10.0.0.2:5555").is_err());
    }

    #[test]
    fn finds_the_wifi_address() {
        assert_eq!(
            parse_route_src("1.1.1.1 via 192.168.1.1 dev wlan0 src 192.168.1.23 uid 2000 \n    cache"),
            Some("192.168.1.23".into())
        );
        assert_eq!(parse_route_src("RTNETLINK answers: Network is unreachable"), None);
        assert_eq!(
            parse_inet("    inet 192.168.1.23/24 brd 192.168.1.255 scope global wlan0"),
            Some("192.168.1.23".into())
        );
        assert_eq!(parse_inet("Device \"wlan0\" does not exist."), None);
    }
}
