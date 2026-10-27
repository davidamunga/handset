# 0027. Build the website with Vite, React and Tailwind

- Status: accepted
- Date: 2026-10-27

## Context

Handset has a domain (handsetapp.com) and needs a home: what it does, screenshots, where to download it, and
the credit that scrcpy, Inter and the author are owed. It should live next to the app so the pictures and the
words stay in step with what ships.

## Decision

The site is a small single page in `site/`, built with Vite, React, TypeScript and Tailwind CSS 4. It is its
own npm package, apart from the Changesets setup at the root, and nothing in it is part of a release.

- It shows the same pictures as the README. `scripts/sync-assets.mjs` copies them (and the icon and the Open
  Graph image) from `docs/images` and `app/packaging/icon` into the git-ignored `site/public/shared`, so there
  is one copy of each in the repository.
- The download buttons ask the GitHub API for the latest release once the page loads and link straight to the
  macOS and Windows files. Without an answer they link to the releases page, so the page works with no network
  call succeeding.
- Fonts are installed from npm (Inter and Bricolage Grotesque, both SIL OFL) and served by the site itself, so
  no request goes to a font host.
- Colour is the app's: greys, with Android green and iOS blue (the two echo phones of the logo) as the only
  colours. The logo's eyes follow the pointer, as they glance about on the connect screen.
- Credit is on the page and in the footer: the author, scrcpy (Genymobile and Romain Vimont, Apache-2.0), the
  fonts, a statement that Handset is independent, and the usual trademark notice for Android, Apple and
  Microsoft.

Where the site is hosted is not decided here.

## Consequences

- A second toolchain (Node) is needed to work on the site, but not to build or release the app.
- A screenshot changed in the README changes on the site at its next build.
- The site shows a version number only if the GitHub API answers; it never hard-codes one.

## Alternatives considered

- **Astro or plain HTML:** lighter for a page this size, but React was asked for, and the logo and the download
  card are small components worth having.
- **Committing a copy of each picture under `site/`:** simpler to build, but two copies drift apart.
- **Hard-coding the download links:** the file names carry the version, so they would break at each release.
