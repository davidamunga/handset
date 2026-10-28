# Publish to the AUR

Two Arch packages live in `app/packaging/linux/`:

| AUR package | PKGBUILD | What it installs |
|---|---|---|
| `handset` | `app/packaging/linux/PKGBUILD` | Builds the release tag from source with Cargo |
| `handset-bin` | `app/packaging/linux/handset-bin/PKGBUILD` | The prebuilt `Handset-<version>-linux-<arch>.tar.gz` from the GitHub release |

Both install `/usr/bin/handset` and provide `handset`, so only one can be installed at a time. Each folder
keeps a `.SRCINFO` next to its PKGBUILD; the AUR reads that, not the PKGBUILD.

The AUR push needs the maintainer's own AUR account with an SSH key added to it. Do it on an Arch machine
(or an `archlinux` container) with `base-devel`, `pacman-contrib` (for `updpkgsums`) and `namcap`.

## First publish

Do this once per package. The AUR creates the repository on the first push.

```sh
git clone ssh://aur@aur.archlinux.org/handset.git aur-handset
cp app/packaging/linux/PKGBUILD app/packaging/linux/.SRCINFO aur-handset/
cd aur-handset
makepkg -si && namcap PKGBUILD *.pkg.tar.zst   # build, install and lint before publishing
git add PKGBUILD .SRCINFO
git commit -m "Initial upload: handset 0.8.1"
git push
```

For `handset-bin`, clone `ssh://aur@aur.archlinux.org/handset-bin.git` and copy from
`app/packaging/linux/handset-bin/`. Its checksums are `SKIP` until a release ships the Linux tarballs: fill
them in first (see below), the AUR expects real ones.

## Each release

Once the release is published on GitHub (both packages download from it):

1. In `app/packaging/linux/PKGBUILD` and `app/packaging/linux/handset-bin/PKGBUILD`, set `pkgver` to the new
   version and `pkgrel=1`. Bump only `pkgrel` when the PKGBUILD changes but the version does not.
2. Update the checksums. In each folder run `updpkgsums`; for `handset-bin` run it on x86_64 and it
   fills both `sha256sums_x86_64` and `sha256sums_aarch64`. Without `pacman-contrib`, hash by hand:

   ```sh
   curl -sL https://github.com/davidamunga/handset/archive/refs/tags/v<version>.tar.gz | sha256sum
   curl -sL https://github.com/davidamunga/handset/releases/download/v<version>/SHA256SUMS-linux
   ```

   Check that `SHA256SUMS-linux` lists both Linux tarballs and that the hashes match what you downloaded.
3. Regenerate `.SRCINFO` in each folder: `makepkg --printsrcinfo > .SRCINFO`.
4. Build and lint: `makepkg -f && namcap PKGBUILD *.pkg.tar.zst`.
5. Commit both folders to this repository (`chore(packaging): update the AUR packages to <version>`).
6. Copy each PKGBUILD and `.SRCINFO` into its AUR clone, then `git commit -m "Update to <version>"` and
   `git push` in each.

Users then get it with `yay -Syu` (or their AUR helper of choice).
