//! The phones Handset connected to over Wi-Fi, newest first, so they can be reconnected with one click.
//! Stored as `~/.config/handset/recent`, one phone per line: `ip:port<TAB>name<TAB>unix seconds`.

use std::path::{Path, PathBuf};

const MAX: usize = 5;

#[derive(Debug, Clone, PartialEq)]
pub struct Recent {
    pub addr: String,
    pub name: String,
    /// Unix seconds when it was last connected.
    pub at: u64,
}

fn dir() -> PathBuf {
    crate::tools::home().join(".config/handset")
}

pub fn load() -> Vec<Recent> {
    load_from(&dir())
}

/// Remember `addr` as the most recently used phone. Errors are ignored: this is a convenience.
pub fn remember(addr: &str, name: &str) {
    let _ = remember_in(&dir(), addr, name, now());
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn load_from(dir: &Path) -> Vec<Recent> {
    std::fs::read_to_string(dir.join("recent")).map(|t| parse(&t)).unwrap_or_default()
}

fn remember_in(dir: &Path, addr: &str, name: &str, at: u64) -> std::io::Result<()> {
    let list = add(load_from(dir), Recent { addr: addr.to_string(), name: name.to_string(), at });
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join("recent"), render(&list))
}

fn parse(text: &str) -> Vec<Recent> {
    text.lines()
        .filter_map(|l| {
            let mut it = l.split('\t');
            let (addr, name, at) = (it.next()?, it.next()?, it.next()?.trim().parse().ok()?);
            (addr.contains(':') && !addr.is_empty()).then(|| Recent {
                addr: addr.to_string(),
                name: name.to_string(),
                at,
            })
        })
        .collect()
}

fn render(list: &[Recent]) -> String {
    list.iter().map(|r| format!("{}\t{}\t{}\n", r.addr, r.name.replace(['\t', '\n'], " "), r.at)).collect()
}

/// `new` goes first; an older entry for the same address is replaced; only the newest `MAX` are kept.
fn add(mut list: Vec<Recent>, new: Recent) -> Vec<Recent> {
    list.retain(|r| r.addr != new.addr);
    list.insert(0, new);
    list.truncate(MAX);
    list
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(addr: &str, name: &str, at: u64) -> Recent {
        Recent { addr: addr.into(), name: name.into(), at }
    }

    #[test]
    fn newest_first_without_duplicates() {
        let list =
            add(vec![r("10.0.0.2:5555", "Pixel", 1), r("10.0.0.3:5555", "Nokia", 2)], r("10.0.0.2:5555", "Pixel 8", 3));
        assert_eq!(list, vec![r("10.0.0.2:5555", "Pixel 8", 3), r("10.0.0.3:5555", "Nokia", 2)]);
    }

    #[test]
    fn keeps_only_the_newest_few() {
        let mut list = Vec::new();
        for i in 0..8 {
            list = add(list, r(&format!("10.0.0.{i}:5555"), "P", i));
        }
        assert_eq!(list.len(), MAX);
        assert_eq!(list[0].at, 7);
    }

    #[test]
    fn round_trips_and_skips_bad_lines() {
        let list = vec![r("10.0.0.2:5555", "UNIWA W888", 5), r("10.0.0.9:41789", "Pixel\t8", 4)];
        let text = render(&list);
        let back = parse(&format!("{text}garbage\nno-port\tx\t1\n"));
        assert_eq!(back.len(), 2);
        assert_eq!(back[0], list[0]);
        assert_eq!(back[1].name, "Pixel 8");
    }

    #[test]
    fn saves_and_loads_a_file() {
        let dir = std::env::temp_dir().join(format!("handset-recent-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(load_from(&dir).is_empty());
        remember_in(&dir, "192.168.0.20:5555", "UNIWA W888", 10).unwrap();
        remember_in(&dir, "192.168.0.31:5555", "Pixel", 20).unwrap();
        let list = load_from(&dir);
        assert_eq!(
            list.iter().map(|r| r.addr.as_str()).collect::<Vec<_>>(),
            ["192.168.0.31:5555", "192.168.0.20:5555"]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
