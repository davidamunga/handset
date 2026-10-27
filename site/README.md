# Handset website

The page at handsetapp.com: what Handset does, screenshots, downloads and credits. Vite, React, TypeScript,
TanStack Router and Tailwind CSS 4. Routes are the files in `src/routes` (home, `/changelog`, `/terms`, `/privacy`). `/changelog` shows
the repository's `CHANGELOG.md`. See [ADR 0027](../docs/adr/0027-build-the-website-with-vite-react-and-tailwind.md).

```sh
cd site
npm install
npm run dev      # copies the shared pictures, then serves http://localhost:5173
npm run build    # builds into site/dist, adds a page per legal URL, and type-checks
```

The pictures, the icon and the Open Graph image come from `docs/images` and `app/packaging/icon`
(`scripts/sync-assets.mjs` copies them into `public/shared`, which is git-ignored). Change them there.

The download buttons read the latest release from the GitHub API, so there is no version number to update.
