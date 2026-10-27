---
"handset": minor
---

The update banner on Linux now knows how Handset was installed. An Arch package copies `yay -S`, `paru -S` or the AUR's `makepkg` steps, a `cargo install` copies the `cargo install ... --force` command (on macOS too), the Nix package is told to update through Nix, and an unpacked release tarball (next to its `handset.desktop`, or in `~/.local/bin`) installs the update itself: it checks the signed `SHA256SUMS-linux`, swaps only the `handset` program and puts the old one back if the new one does not start. A `cargo install` on Windows is no longer mistaken for an unzipped release.
