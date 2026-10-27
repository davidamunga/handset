# Handset website

The page at handsetapp.com: what Handset does, screenshots, downloads and credits. Vite, React, TypeScript and
Tailwind CSS 4. See [ADR 0027](../docs/adr/0027-build-the-website-with-vite-react-and-tailwind.md).

```sh
cd site
npm install
npm run dev      # copies the shared pictures, then serves http://localhost:5173
npm run build    # type-checks and builds into site/dist
```

The pictures, the icon and the Open Graph image come from `docs/images` and `app/packaging/icon`
(`scripts/sync-assets.mjs` copies them into `public/shared`, which is git-ignored). Change them there.

The download buttons read the latest release from the GitHub API, so there is no version number to update.
