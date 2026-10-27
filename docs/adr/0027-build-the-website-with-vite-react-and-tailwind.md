# 0027. Build the website with Vite, React, TanStack Router and Tailwind

- Status: accepted
- Date: 2026-10-27

## Context

Handset has a domain (handsetapp.com) and needs a home: what it does, screenshots, where to download it, and
the credit that scrcpy, Inter and the author are owed. It should live next to the app so the pictures and the
words stay in step with what ships.

## Decision

The site is in `site/`, built with Vite, React, TypeScript, TanStack Router and Tailwind CSS 4. It is its own
npm package, apart from the Changesets setup at the root, and nothing in it is part of a release.

- Four routes, in `src/routes` (file-based, the tree in `routeTree.gen.ts` is generated and committed): the
  home page, `/changelog` (the repository's `CHANGELOG.md`, read at build time and rendered with
  react-markdown, so it never needs copying), `/terms` and `/privacy`. The terms and the privacy policy describe what the app and the site
  really do (no account, no telemetry, one update check to GitHub), so they are to be kept in step with the app.
- It is a single-page app. `scripts/route-pages.mjs` runs after the build and writes a copy of `index.html` for
  each legal URL with its own title, description and address, plus a `_redirects` fallback, so link previews read
  the right words and a static host needs no rewrite rule of its own.
- Search and sharing: each page has its own title, description, canonical address and social card text (the
  post-build step writes them into a copy of `index.html` per URL, and writes `sitemap.xml` and `robots.txt`),
  and the home page carries `SoftwareApplication` structured data and the author's handle for the card.
- Credit for the author (David Amunga, @davidamunga_) is on the page, in the footer and, for the app, in the
  **About Handset** item of its menu. The wallpaper in the frame screenshot is the macOS Ventura desktop picture
  (© Apple Inc.), used only as a backdrop in screenshots and credited on the page; the original file is not
  in the repository. If that ever becomes a problem, any other picture can be restyled in its place.
- Screenshots are of Handset itself: the connect screens are rendered by the app's code, and the mirroring
  and editor pictures were taken of the app mirroring a stock Android emulator, so no one's own phone is shown.

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
- Without prerendering, the text of a page is drawn by JavaScript. The legal pages could be rendered ahead of
  time later (TanStack Start can do it) if that matters to a search engine or an app-store form.
- The site shows a version number only if the GitHub API answers; it never hard-codes one.

## Alternatives considered

- **Astro or plain HTML:** lighter for a page this size, but React and TanStack Router were asked for, and the logo
  and the download card are small components worth having.
- **Committing a copy of each picture under `site/`:** simpler to build, but two copies drift apart.
- **Hard-coding the download links:** the file names carry the version, so they would break at each release.
