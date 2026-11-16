//! The helper on the phone that describes the screen quickly (`helper/` holds its source).
//!
//! Android's `uiautomator dump` takes about three seconds, nearly all of it starting up. The helper is a tiny
//! program that stays running on the phone and answers a request in a few hundredths of a second. Handset
//! pushes it with adb the way it pushes the scrcpy server, talks to it over the adb shell's standard input
//! and output, and stops it when it is done. Anything that goes wrong is the caller's cue to use `uiautomator`.

use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::time::{Duration, Instant};

const JAR: &[u8] = include_bytes!("../assets/handset-helper.jar");
/// The first answer includes starting the phone's runtime.
const START_WITHIN: Duration = Duration::from_secs(20);
const ANSWER_WITHIN: Duration = Duration::from_secs(10);

/// Why a request got no description.
#[derive(Debug, PartialEq)]
pub enum Fault {
    /// The helper answered that it could not: the phone is in a state it cannot describe.
    Refused(String),
    /// The helper is gone or stuck; start it again or give up on it.
    Broken(String),
}

/// The helper ends itself after a minute of nothing; one that has been idle for most of that is replaced, not asked.
const STALE_AFTER: Duration = Duration::from_secs(45);

pub struct Helper {
    serial: String,
    last_used: Instant,
    child: Child,
    stdin: ChildStdin,
    replies: Receiver<Result<Vec<u8>, String>>,
}

impl Helper {
    /// Where the helper goes on the phone. The name holds the jar's checksum, so every Handset that wants this
    /// version shares one copy, and a copy that is running is never pushed over (that makes it abort).
    fn device_path() -> String {
        use sha2::{Digest, Sha256};
        let sum = Sha256::digest(JAR);
        let short: String = sum.iter().take(4).map(|b| format!("{b:02x}")).collect();
        format!("/data/local/tmp/handset-helper-{short}.jar")
    }

    /// Put the helper on the phone (unless this version is already there) and start it.
    pub fn start(serial: &str) -> Result<Helper, String> {
        let path = Self::device_path();
        let there =
            crate::adb::run(serial, &["shell", "stat", "-c", "%s", &path]).ok().and_then(|n| n.parse::<usize>().ok());
        if there != Some(JAR.len()) {
            let local = std::env::temp_dir().join(format!("handset-helper-{}.jar", std::process::id()));
            std::fs::write(&local, JAR).map_err(|e| e.to_string())?;
            let pushed = crate::adb::run(serial, &["push", local.to_str().unwrap_or_default(), &path]);
            let _ = std::fs::remove_file(&local);
            pushed.map_err(|e| e.to_string())?;
        }
        let mut cmd = crate::adb::command(serial);
        cmd.args(["shell", &format!("CLASSPATH={path}"), "app_process", "/", "dev.handset.Helper"]);
        Self::spawn(serial, cmd)
    }

    /// Run `cmd`, which speaks the helper's protocol, and check that it answers.
    fn spawn(serial: &str, mut cmd: Command) -> Result<Helper, String> {
        let mut child = cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;
        let (stdin, stdout) = (child.stdin.take().ok_or("no input")?, child.stdout.take().ok_or("no output")?);
        let (send, replies) = channel();
        // read on a thread, so that a helper that goes quiet can be given up on
        std::thread::spawn(move || {
            let mut out = BufReader::new(stdout);
            loop {
                let mut head = String::new();
                if out.read_line(&mut head).unwrap_or(0) == 0 {
                    let _ = send.send(Err("the helper ended".into()));
                    return;
                }
                let head = head.trim_end();
                let reply = if let Some(n) = head.strip_prefix("ok ") {
                    let n: usize = n.parse().unwrap_or(usize::MAX);
                    let mut body = Vec::new();
                    if n > 64 << 20 || out.by_ref().take(n as u64).read_to_end(&mut body).unwrap_or(0) != n {
                        let _ = send.send(Err("the helper's answer was cut short".into()));
                        return;
                    }
                    Ok(body)
                } else {
                    Err(format!("refused: {}", head.strip_prefix("err ").unwrap_or(head)))
                };
                if send.send(reply).is_err() {
                    return;
                }
            }
        });
        let mut helper = Helper { serial: serial.to_string(), last_used: Instant::now(), child, stdin, replies };
        match helper.ask("ping", START_WITHIN) {
            Ok(_) => Ok(helper),
            Err(Fault::Refused(e) | Fault::Broken(e)) => Err(e),
        }
    }

    pub fn serial(&self) -> &str {
        &self.serial
    }

    /// It has been quiet long enough that it may be about to end by itself.
    pub fn is_stale(&self) -> bool {
        self.last_used.elapsed() > STALE_AFTER
    }

    /// The screen as `uiautomator dump` XML, with the size of the screen on a line before it.
    pub fn dump(&mut self) -> Result<String, Fault> {
        let body = self.ask("dump", ANSWER_WITHIN);
        self.last_used = Instant::now();
        let body = body?;
        String::from_utf8(body).map_err(|_| Fault::Broken("the helper's answer was not text".into()))
    }

    fn ask(&mut self, request: &str, within: Duration) -> Result<Vec<u8>, Fault> {
        writeln!(self.stdin, "{request}").and_then(|_| self.stdin.flush()).map_err(|e| Fault::Broken(e.to_string()))?;
        match self.replies.recv_timeout(within) {
            Ok(Ok(body)) => Ok(body),
            Ok(Err(e)) if e.starts_with("refused: ") => Err(Fault::Refused(e["refused: ".len()..].to_string())),
            Ok(Err(e)) => Err(Fault::Broken(e)),
            Err(RecvTimeoutError::Timeout) => Err(Fault::Broken("the helper did not answer in time".into())),
            Err(RecvTimeoutError::Disconnected) => Err(Fault::Broken("the helper ended".into())),
        }
    }
}

impl Drop for Helper {
    fn drop(&mut self) {
        let _ = writeln!(self.stdin, "quit");
        let _ = self.stdin.flush();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn fake(script: &str) -> Command {
        let mut c = Command::new("sh");
        c.args(["-c", script]);
        c
    }

    const WELL_BEHAVED: &str = r#"while read l; do case "$l" in
        ping) echo "ok 0";;
        dump) printf "ok 12\nhello, world";;
        quit) exit 0;;
        *) echo "err what is $l";;
    esac; done"#;

    #[test]
    fn the_jar_is_the_one_whose_checksum_is_recorded_and_has_a_shared_name() {
        use sha2::{Digest, Sha256};
        let sum: String = Sha256::digest(JAR).iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(sum, include_str!("../assets/handset-helper.sha256").trim(), "rebuild it with helper/build.sh");
        assert_eq!(Helper::device_path(), format!("/data/local/tmp/handset-helper-{}.jar", &sum[..8]));
    }

    #[test]
    fn it_answers_requests_and_reports_what_it_refuses() {
        let mut h = Helper::spawn("S", fake(WELL_BEHAVED)).unwrap();
        assert_eq!(h.dump(), Ok("hello, world".to_string()));
        assert_eq!(h.dump(), Ok("hello, world".to_string()));
        assert_eq!(h.ask("nonsense", Duration::from_secs(2)), Err(Fault::Refused("what is nonsense".into())));
        assert_eq!(h.dump(), Ok("hello, world".to_string()), "still working after a refusal");
        assert_eq!(h.serial(), "S");
    }

    #[test]
    fn one_that_never_starts_or_goes_quiet_is_given_up_on() {
        assert!(Helper::spawn("S", fake("exit 3")).is_err());
        assert!(Helper::spawn("S", fake("echo garbage")).is_err());
        // answers the ping, then goes silent
        let mut h = Helper::spawn("S", fake(r#"read l; echo "ok 0"; sleep 30"#)).unwrap();
        let t = std::time::Instant::now();
        assert_eq!(
            h.ask("dump", Duration::from_millis(300)),
            Err(Fault::Broken("the helper did not answer in time".into()))
        );
        assert!(t.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn one_that_dies_is_noticed() {
        let mut h = Helper::spawn("S", fake(r#"read l; echo "ok 0"; read l; exit 0"#)).unwrap();
        assert!(matches!(h.dump(), Err(Fault::Broken(_))));
        assert!(matches!(h.dump(), Err(Fault::Broken(_))));
    }

    #[test]
    fn a_cut_short_answer_is_not_taken_for_a_description() {
        let mut h = Helper::spawn("S", fake(r#"read l; echo "ok 0"; read l; printf "ok 50\nshort""#)).unwrap();
        assert!(matches!(h.dump(), Err(Fault::Broken(_))));
    }
}
