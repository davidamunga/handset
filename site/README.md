# Handset website

The page at handsetapp.com: what Handset does, screenshots, downloads and credits. Vite, React, TypeScript,
TanStack Router and Tailwind CSS 4. Routes are the files in `src/routes` (home, `/agents`, `/faq`, `/changelog`, `/terms`, `/privacy`). `/changelog` shows
the repository's `CHANGELOG.md`. See [ADR 0027](../docs/adr/0027-build-the-website-with-vite-react-and-tailwind.md).

```sh
cd site
npm install
npm run dev      # copies the shared pictures, then serves http://localhost:5173
npm run build    # builds into site/dist, adds a page per legal URL, and type-checks
```

The pictures, the icon and the Open Graph image come from `docs/images` and `app/packaging/icon`
(`scripts/sync-assets.mjs` copies them into `public/shared`, which is git-ignored). Change them there.

The download buttons read the releases from the GitHub API, so there is no version number to update. The same
request gives the download counts shown beside each system: installable files only, summed over every release.

## The AI agents guide (`/agents`)

The guide's words are in `src/content/agents.md`; `src/content/agents.ts` holds its title, description and dates and
reads the table of contents, the agents to jump to and the questions out of the Markdown (the questions also become
the page's FAQ structured data). The build also writes the guide as HTML: `vite build --ssr src/prerender.tsx`
makes `dist-ssr/prerender.js`, and `scripts/route-pages.mjs` puts what it renders, with the page's metadata and
JSON-LD, into `dist/agents/index.html`, so a crawler that runs no scripts reads the guide. Check the result with
`npm run build` and look at `dist/agents/index.html`. Each client's settings in the guide come from that client's
own documentation: when one changes, change its section. The version in "Before you start" has to be a release
that includes `handset mcp`. See [ADR 0049](../docs/adr/0049-set-the-site-in-allo-and-add-a-prerendered-agents-guide.md).

## Fonts

The page is set in Allo (`allo-font`, the `friendly` cut, SIL Open Font License; the licence is served at
`/fonts/Allo-OFL.txt`) and code in Martian Mono. Both are served from the site itself.
