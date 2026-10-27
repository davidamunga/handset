# Release a version

Releases use [Changesets](https://github.com/changesets/changesets). Handset is a Rust crate, so the
root `package.json` exists only to carry the version; `scripts/sync-version.mjs` copies it into
`app/Cargo.toml` and `app/Cargo.lock`, which is where the app and its bundle read it from.

## Day to day: add a changeset

Any pull request that changes what a user sees adds a changeset:

```sh
npm install        # once
npm run changeset  # pick patch, minor or major and write the summary
```

Commit the generated `.changeset/*.md` with the pull request. Docs, tests, CI and behaviour-neutral
refactors do not need one. While Handset is `0.x`, use `minor` for anything that breaks existing use.

## Cutting a release

A release happens only when you start it. Merging a version pull request does not release anything.

1. Pushes to `main` run the `version` workflow. While changesets are pending it opens or updates a pull
   request titled `chore(release): version packages`. It bumps `package.json`, `app/Cargo.toml` and
   `app/Cargo.lock` and writes the new section of `CHANGELOG.md`.
2. Read that pull request. Edit `CHANGELOG.md` there if a summary needs rewording, then merge it. Nothing
   is published yet; `main` just carries the new version.
3. When you want to ship it, run the `release` workflow from `main`: Actions > release > Run workflow, or

   ```sh
   gh workflow run release.yml
   ```

4. The workflow first checks that `v<version>` does not exist yet and that `CHANGELOG.md` has a section
   for it, then builds the release files on macOS with `scripts/package-release.sh`
   (`Handset-X.Y.Z-macos.zip`, the universal app bundle; `handset-X.Y.Z-macos.tar.gz`, the command line
   tool with the licence and notice; and `SHA256SUMS`) and, on Windows, `scripts/package-windows.ps1`
   (`Handset-X.Y.Z-windows.zip`, unsigned, and `SHA256SUMS-windows`). It then tags `vX.Y.Z`, creates a normal GitHub
   release with the changelog section as its notes and the files attached, bumps the Homebrew cask, and
   opens a pull request that points `bucket/handset.json` (the Scoop manifest, this repository is the bucket) at the
   new Windows zip. `main` only takes pull requests, so merge that one; Scoop users get the release once you do.
5. Check the release page. With the signing secrets set (below) the app is Developer ID signed,
   notarised and stapled, so it opens without a Gatekeeper warning. Without them the files are
   ad-hoc signed and Gatekeeper blocks a downloaded copy until it is opened with right-click, Open
   (or `xattr -dr com.apple.quarantine`).

Releases are normal releases, not pre-releases. To rehearse one first, run the separate `release-dry-run`
workflow (Actions > release-dry-run > Run workflow, or `gh workflow run release-dry-run.yml`, optionally with
`--ref <branch>`): it builds, signs and notarises the macOS files and builds the Windows zip, and keeps them as the
`release-files` and `release-files-windows` artifacts. It tags, publishes and changes nothing else. Both workflows
build through the shared `build-release-files.yml`, so a rehearsal builds exactly what a release would.

## One-time repository settings

- Settings > Actions > General > Workflow permissions: **Read and write**, and tick **Allow GitHub
  Actions to create and approve pull requests**. Without it the workflow cannot open the version PR.
- Tags created with the default `GITHUB_TOKEN` do not start other workflows. The release workflow does not
  depend on that: it builds and publishes in one run that you start by hand.

## Signing and notarising

`scripts/package-release.sh` signs with the hardened runtime when `CODESIGN_IDENTITY` is set, and
notarises (and staples the app) when notary credentials are set too. Both the app and the command line
tool are signed; one notarisation submission covers both. Without any of it the build is ad hoc.

You need a paid Apple Developer account, a **Developer ID Application** certificate, and notarisation
credentials. Keep all of it out of the repository.

Locally:

```sh
# one-off: store notarisation credentials in the keychain (prompts for an app-specific password)
xcrun notarytool store-credentials handset-notary --apple-id you@example.com --team-id TEAMID

CODESIGN_IDENTITY="Developer ID Application: Your Name (TEAMID)" NOTARY_PROFILE=handset-notary \
  scripts/package-release.sh
```

If the keychain holds several certificates with the same name, pass the SHA-1 hash from
`security find-identity -v -p codesigning` instead of the name.

In CI, add these repository secrets (Settings > Secrets and variables > Actions). The build job reads
them and falls back to an ad-hoc build when they are missing:

| Secret | Content |
|---|---|
| `MACOS_CERT_P12` | The Developer ID Application certificate with its private key, exported from Keychain Access as `.p12`, base64 encoded (`base64 -i cert.p12 \| pbcopy`) |
| `MACOS_CERT_PASSWORD` | The password chosen when exporting the `.p12` |
| `MACOS_CODESIGN_IDENTITY` | The certificate name, `Developer ID Application: Your Name (TEAMID)` |
| `NOTARY_KEY_P8` | An App Store Connect API key (`AuthKey_XXXX.p8`, base64 encoded) |
| `NOTARY_KEY_ID` | That key's ID |
| `NOTARY_ISSUER` | The issuer ID shown on the App Store Connect keys page |

### Homebrew cask

After the binaries are attached, the `homebrew` job bumps `version` and `sha256` in `Casks/handset.rb` of
[`davidamunga/homebrew-tap`](https://github.com/davidamunga/homebrew-tap) and pushes the change, so
`brew upgrade --cask handset` sees the new release. It needs one more repository secret:

| Secret | Content |
|---|---|
| `TAP_GITHUB_TOKEN` | A fine-grained personal access token with **Contents: Read and write** on the `homebrew-tap` repository only (GitHub > Settings > Developer settings > Fine-grained tokens). The default `GITHUB_TOKEN` cannot write to another repository. |

Without the secret the job prints a notice and does nothing, and the cask has to be edited by hand.

Check a finished release with `spctl -a -vv Handset.app` (expect `accepted`, `source=Notarized Developer ID`)
and `xcrun stapler validate Handset.app`.

## Building the files locally

`scripts/package-release.sh` writes the same files to `dist/release/` (needs both Rust targets,
`rustup target add aarch64-apple-darwin x86_64-apple-darwin`).

## If something goes wrong

- *The version pull request is stale:* push anything to `main`; the `version` workflow rebuilds it.
- *The release stops with "already released":* the version in `package.json` already has a tag. Merge a version
  pull request (it needs a changeset) to move to the next one.
- *A release is wrong after publishing:* edit or delete the GitHub release and the `vX.Y.Z` tag by hand, fix
  the problem, and run the workflow again.
- *Wrong bump:* edit the changeset file in the version pull request's base (or add a new changeset with
  the bigger bump) and let the pull request regenerate.
- *Versions disagree:* run `npm run version` locally with no changesets pending: it only re-syncs
  `app/Cargo.toml` from `package.json`.

## winget

`app/packaging/winget/` holds the winget manifests for the latest release. Unlike Homebrew and Scoop, winget
takes manifests only through a pull request to [microsoft/winget-pkgs](https://github.com/microsoft/winget-pkgs),
and the release workflow does not do that yet.

First submission (once): copy the three files into `manifests/d/davidamunga/Handset/<version>/` in a fork of
winget-pkgs, run `winget validate --manifest <folder>`, then open the pull request and answer its checks.

Each release after that, from a Windows machine with a GitHub token:

```powershell
wingetcreate update davidamunga.Handset --version X.Y.Z `
  --urls https://github.com/davidamunga/handset/releases/download/vX.Y.Z/Handset-X.Y.Z-windows.zip `
  --submit
```

It downloads the zip, fills in the hash and opens the pull request. Then update the files in
`app/packaging/winget/` to match (the version, the `RelativeFilePath` folder, the URL and the hash).
