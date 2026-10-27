# Updates

Handset looks for a newer release on every start. What happens next depends on how you installed it, because a
package manager should be the one to update what it installed.

## What you see

On every start, Handset asks GitHub for the latest release. If it is newer than the version you run, a blue
banner across the bottom of the phone says so, and a green update button appears in the toolbar, above the menu
button. The banner stays until you click it or its **X**; the toolbar button stays as long as there is an update.
Clicking either does the same thing:

| You installed with | The banner says | A click |
|---|---|---|
| Homebrew | `HANDSET 0.6.0 IS OUT` / `CLICK TO COPY: BREW UPGRADE --CASK HANDSET` | copies `brew upgrade --cask handset` |
| Scoop | `HANDSET 0.6.0 IS OUT` / `CLICK TO COPY: SCOOP UPDATE HANDSET` | copies `scoop update handset` |
| winget | `HANDSET 0.6.0 IS OUT` / `CLICK TO COPY: WINGET UPGRADE DAVIDAMUNGA.HANDSET` | copies `winget upgrade davidamunga.Handset` |
| A downloaded `Handset.app` (macOS) or an unzipped release (Windows) | `HANDSET 0.6.0 IS OUT` / `CLICK TO INSTALL` | installs the update (below) |
| Anything else (`cargo install`, a build of your own) | `HANDSET 0.6.0 IS OUT` / `CLICK TO VIEW` | opens the download page |

The banner's **X** works like **Remind me in 3 days**: the banner stays away for three days, but the toolbar
button is still there if you change your mind.

Handset tells these apart from where it is running (a Homebrew install is the app in `/Applications` that
Homebrew has a Caskroom record for).

The right-click menu always has the update while there is one: **Install Handset X and Restart** (downloaded
installs) or **Copy "..." to Update to X** (package managers) or **Download Handset X...**, then
**What's New in X...**, **Skip X** and **Remind Me in 3 Days**.

- **Skip** keeps that version quiet from then on; a newer version is announced again.
- **Remind me in 3 days** keeps the banner and the toolbar button away for three days.
- **Check for Updates...** always answers, even for a skipped version.
- **Check for Updates on Start** (or `--no-update-check`, or `update-check=off` in the settings file) turns the automatic
  check off.

The check is one request to `api.github.com` through `curl`, with only a `handset/<version>` User-Agent.

## Installing an update in place

For a downloaded app or an unzipped release, **Install Handset X and Restart** does the update for you:

1. It downloads `Handset-X-macos.zip` (or `Handset-X-windows.zip`), the release's `SHA256SUMS` (or
   `SHA256SUMS-windows`) and its signature, into a temporary folder.
2. It checks the signature on the list against the public key built into Handset, and the zip's SHA-256 against the
   signed list. **Nothing that is not signed by this project is installed.**
3. It unpacks the zip and checks that it is the version it announced, that the new program starts, and, on macOS, that
   the app's code-signature seal is intact.
4. It quits. A small helper waits for Handset to exit, puts the new app in place of the old one, starts it, and
   removes the old one. If anything goes wrong it puts the old app back and starts that.

It does not start if you are recording, if Handset cannot write where it is installed (for example `/Applications` when
you are not an administrator), or if the release has no signature. In the last two cases Handset opens the download page and
says why. A failed update leaves its log in `handset-update-<id>/update.log` in your temporary folder for a week.

Package-manager installs are never replaced by Handset itself: updating them with `brew`, `scoop` or `winget` keeps
those tools' records right.

## After an update

Handset remembers the version it last ran (`last-version` in `~/.config/handset/settings`). The first start of a
newer version, however it got there (its own installer, `brew`, `scoop` or a download), shows `UPDATED TO 0.6.1`.
The first run that records a version, and a start of the same or an older version, show nothing.

## Checking a download yourself

Put the release files and the `.sig` next to each other and run:

```sh
handset --verify-release <folder>
```

It prints `ok` for each file whose hash is in the signed list and exits with a non-zero code if anything does not match
or the list is not signed.

## For the maintainer

Releases are signed by the release workflow with the key stored as the repository secret `UPDATE_SIGNING_KEY`; the
public half is in `app/src/verify.rs`. A release made without the secret is unsigned: it is announced, but nobody
can install it in place. Making, backing up and rotating the key is in the
[release runbook](runbooks/release.md#update-signing). The design and its reasons are in
[ADR 0024](adr/0024-update-in-place-only-where-nothing-else-manages-the-install.md).

To try an update without a real release, put the zip, `SHA256SUMS` and `SHA256SUMS.sig` (signed with the key the
build trusts) in a folder and start Handset with `HANDSET_UPDATE_BASE=file:///that/folder` (the files are named as
in a release). `HANDSET_UPDATE_CURRENT=0.1.0` makes Handset believe it is an older version, to see the notice.
