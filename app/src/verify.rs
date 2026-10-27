//! Checking that a release really comes from this project: the release's `SHA256SUMS` file is signed with an
//! Ed25519 key (`scripts/update-key.sh`), the app carries the public half, and a file is accepted only when
//! the signature on the list verifies and the file's SHA-256 is in the list. A checksum on its own proves
//! only that a download is intact, not who made it.

use ed25519_dalek::{Signature, VerifyingKey};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::Path;

/// The keys whose signatures are trusted, as 64 hex characters each. More than one allows a new key to be
/// added before an old one is retired.
pub const KEYS: &[&str] = &["44f20c5a3322b0e3b8a9172372ea3f39dee6641a03e7b74121a6ba9a517c9660"];

pub fn parse_hex32(s: &str) -> Option<[u8; 32]> {
    let s = s.trim();
    if s.len() != 64 || !s.is_ascii() {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).ok()?;
    }
    Some(out)
}

/// The keys built into the app.
pub fn trusted_keys() -> Vec<VerifyingKey> {
    keys_from(KEYS)
}

pub fn keys_from(hex: &[&str]) -> Vec<VerifyingKey> {
    hex.iter().filter_map(|h| parse_hex32(h)).filter_map(|b| VerifyingKey::from_bytes(&b).ok()).collect()
}

/// Is `sig` (the raw 64 bytes) a valid signature of `message` by any of `keys`? Strict: it refuses the
/// malleable encodings the plain check would accept.
pub fn verify_with(keys: &[VerifyingKey], message: &[u8], sig: &[u8]) -> bool {
    let Ok(sig) = Signature::from_slice(sig) else { return false };
    keys.iter().any(|k| k.verify_strict(message, &sig).is_ok())
}

/// The hash `sums` lists for `name`.
pub fn expected_hash(sums: &str, name: &str) -> Option<String> {
    parse_sums(sums).into_iter().find(|(_, n)| n == name).map(|(h, _)| h)
}

/// The `(hash, file name)` pairs of a `SHA256SUMS` file: `<hash>  <name>`, or `<hash> *<name>` for binary mode.
pub fn parse_sums(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|line| {
            let (hash, name) = line.trim().split_once(char::is_whitespace)?;
            let name = name.trim_start().trim_start_matches('*');
            let ok = hash.len() == 64 && hash.chars().all(|c| c.is_ascii_hexdigit()) && !name.is_empty();
            ok.then(|| (hash.to_lowercase(), name.to_string()))
        })
        .collect()
}

pub fn sha256_file(path: &Path) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// Check the release files in `dir` against `<sums_name>` and `<sums_name>.sig`: the signature must verify,
/// and every listed file that is present must match its hash. Returns what was checked, or why it failed.
pub fn verify_release_with(keys: &[VerifyingKey], dir: &Path, sums_name: &str) -> Result<Vec<String>, String> {
    let sums_path = dir.join(sums_name);
    let sums = std::fs::read(&sums_path).map_err(|e| format!("{}: {e}", sums_path.display()))?;
    let sig_path = dir.join(format!("{sums_name}.sig"));
    let sig = std::fs::read(&sig_path)
        .map_err(|_| format!("{} is missing: this release is not signed", sig_path.display()))?;
    if !verify_with(keys, &sums, &sig) {
        return Err(format!("the signature on {sums_name} does not verify: do not trust these files"));
    }
    let text = String::from_utf8(sums).map_err(|_| format!("{sums_name} is not text"))?;
    let mut checked = Vec::new();
    for (hash, name) in parse_sums(&text) {
        let path = dir.join(&name);
        if !path.is_file() {
            continue;
        }
        let actual = sha256_file(&path).map_err(|e| format!("{name}: {e}"))?;
        if actual != hash {
            return Err(format!("{name} does not match the signed list: do not trust it"));
        }
        checked.push(name);
    }
    if checked.is_empty() {
        return Err(format!("none of the files listed in {sums_name} is in {}", dir.display()));
    }
    Ok(checked)
}

/// `handset --verify-release <dir>`: check downloaded release files, exit 0 when they are good.
pub fn run(dir: &Path) -> i32 {
    let keys = keys_from(KEYS);
    let mut failed = false;
    let mut found = false;
    for sums in ["SHA256SUMS", "SHA256SUMS-windows"] {
        if !dir.join(sums).is_file() {
            continue;
        }
        found = true;
        match verify_release_with(&keys, dir, sums) {
            Ok(files) => files.iter().for_each(|f| println!("ok      {f}")),
            Err(e) => {
                eprintln!("FAILED  {e}");
                failed = true;
            }
        }
    }
    if !found {
        eprintln!("no SHA256SUMS or SHA256SUMS-windows in {}", dir.display());
        return 1;
    }
    i32::from(failed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    fn signer(seed: u8) -> SigningKey {
        SigningKey::from_bytes(&[seed; 32])
    }

    fn public(sk: &SigningKey) -> Vec<VerifyingKey> {
        vec![sk.verifying_key()]
    }

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("handset-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn the_built_in_keys_are_real_public_keys() {
        assert!(!KEYS.is_empty());
        assert_eq!(keys_from(KEYS).len(), KEYS.len(), "every built-in key must parse as an Ed25519 public key");
    }

    #[test]
    fn hex_is_read_strictly() {
        assert_eq!(parse_hex32(&"ab".repeat(32)), Some([0xab; 32]));
        assert_eq!(parse_hex32(&format!(" {} \n", "00".repeat(32))), Some([0; 32]));
        assert_eq!(parse_hex32("abc"), None);
        assert_eq!(parse_hex32(&"zz".repeat(32)), None);
        assert_eq!(parse_hex32(&"ab".repeat(33)), None);
    }

    #[test]
    fn a_signature_verifies_only_for_its_message_and_key() {
        let (a, b) = (signer(1), signer(2));
        let sig = a.sign(b"the list").to_bytes();
        assert!(verify_with(&public(&a), b"the list", &sig));
        assert!(!verify_with(&public(&a), b"the list!", &sig), "a changed message");
        assert!(!verify_with(&public(&b), b"the list", &sig), "another key");
        let mut bad = sig;
        bad[10] ^= 1;
        assert!(!verify_with(&public(&a), b"the list", &bad), "a damaged signature");
        assert!(!verify_with(&public(&a), b"the list", &sig[..63]), "a short signature");
        assert!(!verify_with(&public(&a), b"the list", b""), "no signature");
        assert!(!verify_with(&[], b"the list", &sig), "no trusted keys");
    }

    #[test]
    fn any_of_several_keys_will_do_so_a_key_can_be_rotated() {
        let (old, new) = (signer(3), signer(4));
        let keys = vec![old.verifying_key(), new.verifying_key()];
        assert!(verify_with(&keys, b"m", &old.sign(b"m").to_bytes()));
        assert!(verify_with(&keys, b"m", &new.sign(b"m").to_bytes()));
    }

    /// A signature made by `openssl` (scripts/update-key.sh) must verify here: the two tools have to agree.
    #[test]
    fn a_signature_made_by_openssl_verifies() {
        const PUBLIC: &str = "0b538336b4c66a50478e56c3cc372fefe23a51c0926523816dbae200c26ef62a";
        const MESSAGE: &[u8] = b"hello\n";
        // openssl pkeyutl -sign -rawin of MESSAGE with the key whose public half is PUBLIC
        const SIG: &str = include_str!("testdata/openssl-hello.sig.hex");
        let sig: Vec<u8> =
            (0..SIG.trim().len() / 2).map(|i| u8::from_str_radix(&SIG.trim()[2 * i..2 * i + 2], 16).unwrap()).collect();
        assert_eq!(sig.len(), 64);
        assert!(verify_with(&keys_from(&[PUBLIC]), MESSAGE, &sig));
        assert!(!verify_with(&keys_from(&[PUBLIC]), b"hello", &sig));
    }

    #[test]
    fn sha256_matches_the_standard_test_vectors() {
        let dir = temp_dir("sha");
        let abc = dir.join("abc");
        std::fs::write(&abc, "abc").unwrap();
        assert_eq!(sha256_file(&abc).unwrap(), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        let empty = dir.join("empty");
        std::fs::write(&empty, "").unwrap();
        assert_eq!(sha256_file(&empty).unwrap(), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        assert!(sha256_file(&dir.join("missing")).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_sums_file_is_read_in_both_styles_and_junk_is_skipped() {
        let h1 = "a".repeat(64);
        let h2 = "B".repeat(64);
        let text = format!(
            "{h1}  Handset-0.5.0-macos.zip\n{h2} *handset-0.5.0-macos.tar.gz\n\nnot a hash  file\n{}  short\n",
            "c".repeat(10)
        );
        let list = parse_sums(&text);
        assert_eq!(expected_hash(&text, "Handset-0.5.0-macos.zip"), Some(h1.clone()));
        assert_eq!(expected_hash(&text, "missing.zip"), None);
        assert_eq!(
            list,
            vec![(h1, "Handset-0.5.0-macos.zip".into()), ("b".repeat(64), "handset-0.5.0-macos.tar.gz".into())]
        );
    }

    /// A release folder: some files, their SHA256SUMS and its signature.
    fn release(dir: &Path, key: &SigningKey, files: &[(&str, &[u8])]) {
        let mut sums = String::new();
        for (name, data) in files {
            std::fs::write(dir.join(name), data).unwrap();
            sums.push_str(&format!("{}  {name}\n", sha256_file(&dir.join(name)).unwrap()));
        }
        std::fs::write(dir.join("SHA256SUMS"), &sums).unwrap();
        std::fs::write(dir.join("SHA256SUMS.sig"), key.sign(sums.as_bytes()).to_bytes()).unwrap();
    }

    #[test]
    fn a_good_release_checks_out_and_lists_what_it_checked() {
        let (dir, key) = (temp_dir("rel-good"), signer(5));
        release(&dir, &key, &[("a.zip", b"first"), ("b.tar.gz", b"second")]);
        let mut done = verify_release_with(&public(&key), &dir, "SHA256SUMS").unwrap();
        done.sort();
        assert_eq!(done, vec!["a.zip", "b.tar.gz"]);
        // a listed file that was not downloaded is fine
        std::fs::remove_file(dir.join("b.tar.gz")).unwrap();
        assert_eq!(verify_release_with(&public(&key), &dir, "SHA256SUMS").unwrap(), vec!["a.zip"]);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_tampered_file_a_tampered_list_or_a_missing_signature_is_refused() {
        let (dir, key) = (temp_dir("rel-bad"), signer(6));
        release(&dir, &key, &[("a.zip", b"first")]);

        std::fs::write(dir.join("a.zip"), b"first, but changed").unwrap();
        let e = verify_release_with(&public(&key), &dir, "SHA256SUMS").unwrap_err();
        assert!(e.contains("a.zip") && e.contains("do not trust"), "{e}");

        release(&dir, &key, &[("a.zip", b"first")]);
        let list = std::fs::read_to_string(dir.join("SHA256SUMS")).unwrap();
        std::fs::write(dir.join("SHA256SUMS"), list.replace('a', "b")).unwrap();
        let e = verify_release_with(&public(&key), &dir, "SHA256SUMS").unwrap_err();
        assert!(e.contains("signature") && e.contains("does not verify"), "{e}");

        release(&dir, &key, &[("a.zip", b"first")]);
        std::fs::remove_file(dir.join("SHA256SUMS.sig")).unwrap();
        let e = verify_release_with(&public(&key), &dir, "SHA256SUMS").unwrap_err();
        assert!(e.contains("not signed"), "{e}");

        release(&dir, &signer(7), &[("a.zip", b"first")]);
        assert!(verify_release_with(&public(&key), &dir, "SHA256SUMS").is_err(), "signed by someone else");

        std::fs::remove_file(dir.join("a.zip")).unwrap();
        release(&dir, &key, &[("a.zip", b"first")]);
        std::fs::remove_file(dir.join("a.zip")).unwrap();
        let e = verify_release_with(&public(&key), &dir, "SHA256SUMS").unwrap_err();
        assert!(e.contains("none of the files"), "{e}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_command_reports_by_exit_code() {
        // no sums at all, or sums the app's own key did not sign, both fail
        let dir = temp_dir("rel-run");
        assert_eq!(run(&dir), 1);
        release(&dir, &signer(8), &[("a.zip", b"x")]);
        assert_eq!(run(&dir), 1, "signed by a key the app does not trust");
        let _ = std::fs::remove_dir_all(dir);
    }
}
