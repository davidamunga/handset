# 0049. Set the site in Allo, and add a pre-rendered guide for AI agents

- Status: accepted
- Date: 2026-11-16

## Context

`handset mcp` ([ADR 0048](0048-offer-the-phone-to-agents-as-an-mcp-server.md)) lets AI agents use the phone, and
nothing on the site said so. People looking for how to connect an agent search for the agent they use (Claude
Code, Cursor, Codex and so on) and for what they want to do, so a short feature row is not enough: each client
has its own settings file and format, and a person following a guide needs exactly the right one.

The site is a client-rendered single page. `scripts/route-pages.mjs` writes a copy of `index.html` for each long
URL with its own title and description, but the body of each copy is empty until scripts run. Search engines
that run scripts can read such a page, slowly; many crawlers, and most AI crawlers, do not run scripts at all.

The site's fonts were Rubik for headlines and Atkinson Hyperlegible Next for text, with Martian Mono for labels
and code ([ADR 0042](0042-set-the-site-in-rubik-atkinson-and-martian-mono.md)). The wish was to set the whole
page in Allo (https://allo.joshpuckett.me).

## Decision

**The font.** We will set the whole site in Allo, and keep a monospace face only for code.

- Allo is by Josh Puckett, free and open source under the SIL Open Font License 1.1, and grew from Inter. It is
  a variable font (weight, roundness, optical size) with a Display design that takes over at large sizes.
- It is taken from the `allo-font` package on npm, whose files were read before use (CSS, `.woff2` fonts and the
  licence, no install scripts), and served from the site itself: no request to a third party, and no visitor
  address sent anywhere.
- The `friendly` cut is used (roundness fixed at 50): 229 KB against 442 KB for the full variable font, and
  the default look is the friendly one. Switching to the full font, to tune roundness, is one import.
- The small capital labels that were in Martian Mono are now in Allo capitals. Martian Mono stays for code
  blocks, commands and inline code, where a monospace face is the point.
- The font file is preloaded on every page, so the words do not jump when it arrives.
- Allo is credited on the page and its licence is served at `/fonts/Allo-OFL.txt`.
- Rubik and Atkinson Hyperlegible Next are removed. Everything else in ADR 0042 (paper, pills, highlighter,
  no gradients) stands.

**The guide.** We will add `/agents`, a guide to using AI agents with Handset, for every agent that can run a
local MCP server, and make it findable.

- **One source.** The words are in `src/content/agents.md`. The page, the HTML written by the build, the table of
  contents, the "jump to your agent" links and the structured data are all made from that file.
- **Pre-rendered.** The build renders the guide to HTML (`vite build --ssr src/prerender.tsx`) and
  `route-pages.mjs` writes it into `dist/agents/index.html`, so a crawler that runs no scripts reads about 2,600
  words, with their headings, commands and questions. The app then takes over from that copy. The guide uses
  plain links, not the router's, so it can be rendered without a router.
- **What search engines are told.** A title under 60 characters, a description of about 150, a canonical
  address, Open Graph and Twitter tags, `og:type` article, and JSON-LD: a TechArticle, a FAQPage made from the
  guide's own questions, and a BreadcrumbList. The sitemap lists it.
- **Each client's settings come from its own documentation**, read for this change: Claude Code (checked against
  the installed `claude mcp add --help`), Claude Desktop, Cursor, VS Code, Windsurf, Cline, Zed, Gemini CLI and
  Codex. Where a client's file location has changed between versions (Windsurf), the guide points to the
  client's own button and not a path.
- **Where it is linked:** the header, the footer, a fifth row in the home page's feature list with a copyable
  command, and a question in the home FAQ.

## Alternatives considered

- **A framework with server rendering (Next, Astro).** Would pre-render every page, but replaces the build of a
  small site for one page's sake. The same script already writes the other pages' metadata.
- **Leave the guide to the repository's docs.** The docs describe the commands; a visitor who is deciding
  whether to try it, or comparing agents, needs the guide on the site, with its own title and address.
- **One page per agent** (`/agents/claude-code`, `/agents/cursor`). More addresses to be found at, and each
  would be short. One long page with an anchor per agent was chosen first; splitting it is possible later.
- **Keep Martian Mono for the labels.** Allo does the labels well in capitals, and one face makes the page calmer.
- **The full variable font.** Twice the size for a roundness control the page does not use.

## Consequences

- The site loads one 229 KB text font and a small mono one (code only), where it loaded two text fonts and the
  mono one.
- The guide has to be kept true: Handset's version in "Before you start" (0.16, the release that includes
  `handset mcp`, to be confirmed when it is released), the tool list, and each client's settings. A client that
  changes its format makes its section wrong until someone notices. The page should not be published before a
  release contains `handset mcp`.
- The page tells people to start Handset with `--ctl` or to edit a settings file, because the app has no menu
  switch for control yet. A menu switch would shorten step 1.
- The other long pages (`/faq`, `/terms`, `/privacy`, `/changelog`) still have an empty body in their HTML until
  scripts run. They could be pre-rendered the same way.
- Not done: screenshots of an agent using the phone, and per-agent pages.
