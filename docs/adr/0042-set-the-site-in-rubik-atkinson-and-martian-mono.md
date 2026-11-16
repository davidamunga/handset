# 0042. Give the site a warm, rounded look in Rubik, Atkinson Hyperlegible Next and Martian Mono

- Status: accepted
- Date: 2026-11-16

## Context

The site (ADR 0027) used Inter and Bricolage Grotesque on cool grey, with plain rounded cards. That is the stock look
of a modern app page, and it did not say anything about Handset. The look of theweekendproject.club (warm paper,
soft pills, big two-tone headlines with a yellow highlighter, small interactive touches) was chosen as the
inspiration: its spirit, not its pieces.

## Decision

- Headlines: Rubik, heavy and tight. Text: Atkinson Hyperlegible Next. Small labels, numbers and code: Martian Mono.
  All three are SIL OFL 1.1 variable fonts from Fontsource, credited on the page. The app keeps Inter (ADR 0017).
- Warm near-white paper, pill buttons, white rounded cards with a thin warm edge, and a yellow highlighter pill
  (`.hl`) on the words that matter. The logo's green and blue are the only other colours. No gradients, glow or
  drop shadows.
- Headlines are two-tone: ink, then a quieter colour (`.fade`).
- Small interactive parts, all plain React state: the hero's highlighted words swap when tapped (and the download
  button follows the chosen system), the "Is Handset for you?" list is tappable, the FAQ opens with `<details>`.
- The hero shows the app, not words about it: a short headline and the download buttons beside a collage of the real
  app (the mirrored phone, the style editor and the share page, each with a one-word label), so the first screen
  already says how it looks and works.
- New home sections: a dark announcement strip with the latest version, a story paragraph, "for you", a short FAQ,
  a closing card and a floating Download pill.
- Component classes (`.btn`, `.card`, `.display`, `.label`, `.hl`, `.fade`) live in the Tailwind components layer so
  a utility can still override them. Routes and the download logic are unchanged.

## Alternatives considered

- **Keep Bricolage Grotesque or Inter:** both are current favourites of generated pages.
- **Copy the reference's fonts and layout:** it is someone else's site; we took the approach and drew our own.
- **A hard-edged, offset-shadow style:** tried first; it read as a different brand's poster.

## Consequences

- Three variable fonts instead of two: about 90 kB of woff2 for the Latin subsets, loaded with `swap`.
- The page has more copy to keep true: the "for you" and FAQ answers repeat claims made elsewhere (iOS is view only,
  Share is on the network only, Linux is experimental), so change them together.
