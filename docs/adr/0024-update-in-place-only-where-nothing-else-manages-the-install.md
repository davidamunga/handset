# 0024. Update in place only where nothing else manages the install, and only to signed releases

- Status: accepted (extended to Linux by [0036](0036-update-linux-installs-by-how-they-were-made.md))
- Date: 2026-10-27

## Context

Handset looked for a newer release once a day and, when there was one, showed a toast that opened the
download page (ADR in `update.rs`'s header). That is the same on every system, but it is not what the person
needs: someone who installed with Homebrew or Scoop should run one command, someone with a downloaded app
should not have to download, unzip and drag it over the old one, and a person who is busy should be able to
say "not this one" or "later". People also install Handset in four ways (Homebrew, Scoop, winget, a download),
and a self-updater that swaps files behind a package manager's back leaves that package manager believing
in a version that is no longer there.

## Decision

Handset works out how it was installed (`install.rs`, from the program's path and, on macOS, Homebrew's
Caskroom) and treats each kind differently:

- **Homebrew, Scoop and winget installs** are never replaced by Handset. The toast and the menu give the
  command (`brew upgrade --cask handset`, `scoop update handset`, `winget upgrade davidamunga.Handset`) and a
  click copies it.
- **A downloaded app on macOS and an unzipped release on Windows** can be updated by Handset itself: it
  downloads the release, checks it, swaps it in and starts the new version (ADR text below, built in steps).
- **Anything else** (`cargo install`, a development build) only gets a link to the download page.

Every update can be skipped (that version stays quiet; the menu still offers it) or postponed for three days,
and a manual **Check for Updates...** always answers.

Nothing is installed unless it is signed. The release workflow signs the release's `SHA256SUMS` files with an
Ed25519 key kept as a repository secret, and Handset carries the public key. The download is accepted only
when the signature verifies and the zip's SHA-256 is in the signed list. A release with no signature is
announced but not installed in place. This protects against a tampered download or a compromised mirror,
which a checksum fetched from the same place cannot.

## Alternatives considered

- **Always open the download page (the old behaviour):** works everywhere, but makes the person do the work,
  and tells a Homebrew user to download an app that `brew upgrade` would have done better.
- **Replace the app for every install source:** simple, but breaks package managers.
- **Use Sparkle (macOS) and a Windows installer framework:** mature, but two different mechanisms, a large
  dependency, and Handset ships a zip, not an installer. One small Rust updater serves both.
- **Verify only the checksum:** protects against a corrupted download, not a malicious one.

## Consequences

- The update key is a long-lived secret: losing it means releasing a version that carries a new public key,
  and only people who update by hand get it. Keeping a backup of the key is part of the release process.
- Windows builds are not code-signed (ADR 0020), so the Ed25519 signature is the only thing that vouches for
  an update there.
- The updater was built in steps: the install-aware notice and skip/remind, then the signing and
  verification, then the download and swap.

## How the in-place update works

`updater.rs` downloads `SHA256SUMS`, its `.sig` and the zip into a temporary folder; verifies the signature and
the zip's hash; unpacks (`ditto` on macOS so the code signature survives, `tar` on Windows); and checks that what
it unpacked is the announced version (the bundle's version, and the new program's own `--version`) and, on macOS,
that the app's `codesign` seal is intact. A signed list that does not name the zip, or a zip whose contents are
an older version, is refused, so a validly signed old release cannot be passed off as a new one.

Only then does it write a helper script and quit. The helper (a shell script on macOS, PowerShell on Windows)
waits for Handset to exit, copies the new app next to the old one, moves the old one aside, puts the new one in
place, starts it, and deletes the old one. If the swap or the start fails, it puts the old one back and starts
that. The update never starts if Handset cannot write where it is installed (for example `/Applications` for a
non-administrator), while a recording is running, or for an install that a package manager owns.

When the folder is not writable the updater does not try to elevate: it opens the download page and says why,
the same fallback as for an unsigned release. The `handset-update-<pid>` folders it leaves in the temporary
folder (the download, the unpacked app and the helper's log) are removed at start once they are a week old, so
the log stays available for a while after a failed update.
