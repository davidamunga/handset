// Built on its own by `vite build --ssr` and run by scripts/route-pages.mjs: the guide as HTML, to write into
// dist/agents/index.html. The page starts from these words and then the app takes over, so what a crawler that
// does not run scripts reads is the same guide, in a plain header and footer with plain links.
import type { ReactNode } from 'react'
import { renderToString } from 'react-dom/server'
import { AgentsGuide } from './components/AgentsGuide'
import { Mark } from './components/Mark'
import { AGENTS_DESCRIPTION, AGENTS_KEYWORDS, AGENTS_TITLE, agentsJsonLd } from './content/agents'
import { AUTHOR, REPO } from './links'

const nav = 'whitespace-nowrap rounded-full px-3.5 py-2 text-[0.95rem] text-ink-2'

function Shell({ children }: { children: ReactNode }) {
  return (
    <>
      <header className="mx-auto flex max-w-6xl 2xl:max-w-7xl items-center justify-between px-5 py-4 sm:px-8">
        <a href="/" className="display flex items-center gap-2.5 text-2xl">
          <Mark className="size-10" follow={false} />
          Handset
        </a>
        <nav aria-label="Main" className="hidden items-center gap-1 md:flex">
          <a href="/#features" className={nav}>What it does</a>
          <a href="/#download" className={nav}>Download</a>
          <a href="/agents" className={nav}>AI agents</a>
          <a href="/changelog" className={nav}>Changelog</a>
          <a href="/faq" className={nav}>FAQ</a>
          <a href={REPO} className={nav}>GitHub</a>
        </nav>
      </header>
      <main id="main">{children}</main>
      <footer className="bg-stage text-stage-text">
        <div className="mx-auto max-w-6xl 2xl:max-w-7xl px-5 py-12 sm:px-8">
          <nav aria-label="Footer" className="flex flex-wrap gap-x-6 gap-y-3">
            <a className="label" href={REPO}>GitHub</a>
            <a className="label" href={`${REPO}/releases`}>Releases</a>
            <a className="label" href="/agents">AI agents</a>
            <a className="label" href="/faq">FAQ</a>
            <a className="label" href="/changelog">Changelog</a>
            <a className="label" href="/terms">Terms</a>
            <a className="label" href="/privacy">Privacy</a>
          </nav>
          <p className="mt-8 text-sm">© 2026 {AUTHOR.name}. Handset is Apache-2.0 licensed.</p>
        </div>
      </footer>
    </>
  )
}

export function renderAgentsPage() {
  return {
    html: renderToString(
      <Shell>
        <AgentsGuide />
      </Shell>,
    ),
    jsonLd: agentsJsonLd(),
    title: AGENTS_TITLE,
    description: AGENTS_DESCRIPTION,
    keywords: AGENTS_KEYWORDS,
  }
}
