# 0036. Update Linux installs by how they were made, and swap only the program of an unpacked tarball

- Status: accepted
- Date: 2026-10-28

## Context

ADR 0024 lets Handset update itself only where nothing else manages the install, and gives a package
manager's install its command. On Linux every install was "anything else", so the notice always opened the
download page, though people install Handset there in four ways: an Arch package (`handset` or `handset-bin`,
in `/usr/bin`), `cargo install` (`~/.cargo/bin`), the Nix package (`/nix/store`) and, now that releases carry
signed `Handset-<version>-linux-<arch>.tar.gz` files, an unpacked tarball. The tarball is put anywhere: left in
its own folder, or its `handset` copied to `~/.local/bin` next to other people's programs.

## Decision

`install.rs` tells the Linux installs apart from the program's path, with the facts it cannot read from the
path found first (so `detect` stays pure):

- **Arch:** `/usr/bin/handset` that `pacman -Qqo` says belongs to `handset` or `handset-bin`. The notice copies
  `yay -S <package>` or `paru -S <package>` when one of them is on `PATH`, otherwise the AUR's clone-and-build
  steps (`git clone https://aur.archlinux.org/<package>.git && cd <package> && makepkg -si`). Without pacman, or
  for a package of another name, it is "anything else".
- **cargo:** `~/.cargo/bin/handset` (on every system; on Windows this also stops a cargo install from being
  taken for an unzipped release). The notice copies the README's `cargo install ... --force`.
- **Nix:** a path in `/nix/store`. Nix setups differ too much for one command, so the notice says to update
  through Nix and opens the release page.
- **An unpacked tarball:** a `handset` with the tarball's `handset.desktop` next to it, or, on Linux,
  `~/.local/bin/handset`. It is updated in place like the macOS app and the Windows zip: the same download,
  signature (`SHA256SUMS-linux` and its `.sig`) and version checks, then a shell helper. The helper replaces
  only the `handset` program (copy next to it, move the old one aside, rename the new one in, start it, and put
  the old one back if it exits at once with an error), and refreshes `handset.desktop`, `handset.png`, `LICENSE`
  and `NOTICE` only where they already sit next to it, so a shared folder such as `~/.local/bin` gets nothing it
  did not have.

A release with no `SHA256SUMS-linux` at all (one from before Linux was a release file) is treated like an
unsigned one: the download page opens.

## Alternatives considered

- **Keep "click to view" for every Linux install:** simple, but it makes the people with a package manager or
  cargo look up a command Handset knows.
- **Replace the whole folder, as on Windows:** wrong for a program copied into `~/.local/bin`, where the folder
  is not Handset's.
- **Ask `pacman` about every path:** a hand-copied `/usr/local/bin/handset` is never a package, and asking only
  about `/usr/bin/handset` keeps the start-up cost to one quick command on Arch.
- **Detect `~/.local/bin` everywhere:** on macOS nothing puts Handset there, and a Linux tarball must never be
  installed over a Mac program, so that rule is Linux-only.

## Consequences

- The AUR commands assume the packages are published under those names (`docs/runbooks/aur.md`).
- A tarball install whose folder is not writable falls back to the download page, as on the other systems.
- The swap is a rename on one file system, so the running program keeps working until Handset quits.
