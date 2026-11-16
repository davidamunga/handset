# 0042. Set the site in Archivo, Atkinson Hyperlegible Next and Martian Mono, and print it flat

- Status: accepted
- Date: 2026-11-16

## Context

The site (ADR 0027) used Inter and Bricolage Grotesque on cool grey, with soft rounded cards. That is the stock
look of a modern app page, and it did not say anything about Handset.

## Decision

- Headlines: Archivo, heavy and widened (its `wdth` axis, 112%), set tight. Text: Atkinson Hyperlegible Next.
  Small labels, numbers and code: Martian Mono. All three are SIL OFL 1.1 variable fonts from Fontsource, and
  credited on the page. The app keeps Inter (ADR 0017); the site no longer uses it.
- Flat, like a printed sticker sheet: warm paper, ink edges (2px), hard offset shadows, no gradients, glow or soft
  shadows. Colours are the two echo phones of the logo (Android green, iOS blue) and one marker yellow.
- Buttons, cards and labels are Tailwind component classes in `index.css` (`.btn`, `.card`, `.display`, `.label`,
  `.marker`), in the components layer so a utility can still override them.
- Content, routes and behaviour are unchanged.

## Alternatives considered

- **Keep Bricolage Grotesque for headlines:** it is a current favourite of generated pages as well.
- **A serif for headlines:** reads as editorial, which Handset is not.

## Consequences

- Three variable fonts instead of two: about 90 kB more of woff2 for the Latin subsets, loaded with `swap`.
- The look is more opinionated; a new section should use the same classes, not new colours.
