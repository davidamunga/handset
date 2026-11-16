// The AI agents guide: its words (agents.md), what search engines are told about it, and what is read out of the
// words for the page (the table of contents, the agents to jump to, the questions). The page, the pre-rendered
// copy of it that the build writes into dist/agents/index.html, and the structured data all read from here.
import guide from './agents.md?raw'
import { AUTHOR, SITE } from '../links'

export const GUIDE_MD: string = guide

export const AGENTS_PATH = '/agents'
export const AGENTS_URL = `${SITE}${AGENTS_PATH}`
/** The tab title and search result title: under 60 characters. */
export const AGENTS_TITLE = 'AI agents on your Android phone: MCP guide | Handset'
export const AGENTS_H1 = 'Let AI agents use your Android phone'
/** The snippet under the result: about 150 characters. */
export const AGENTS_DESCRIPTION =
  "Connect Claude Code, Cursor, VS Code, Codex, Gemini CLI and other AI agents to a real Android phone with Handset's free MCP server. Setup for each."
export const AGENTS_KEYWORDS =
  'MCP server Android, AI agent Android phone, Claude Code Android, Cursor MCP, Codex MCP, Gemini CLI MCP, test Android app with AI, Handset MCP'
export const AGENTS_PUBLISHED = '2026-11-16'
export const AGENTS_UPDATED = '2026-11-16'

/** An id for a heading, the same on the page and in the table of contents. */
export function slug(text: string): string {
  return text
    .toLowerCase()
    .replace(/[^a-z0-9\s-]/g, '')
    .trim()
    .replace(/\s+/g, '-')
}

/** Markdown with its marks taken off: for the descriptions search engines read. */
export function plain(md: string): string {
  return md
    .replace(/\[([^\]]+)\]\([^)]+\)/g, '$1')
    .replace(/[`*_]/g, '')
    .replace(/\s+/g, ' ')
    .trim()
}

export type Heading = { id: string; text: string }
export type Question = { q: string; a: string }

/** The headings and questions in the guide. Lines inside a code block are not headings. */
export function parseGuide(md: string): { toc: Heading[]; agents: Heading[]; faq: Question[] } {
  const toc: Heading[] = []
  const agents: Heading[] = []
  const faq: Question[] = []
  let section = ''
  let question: { q: string; lines: string[] } | null = null
  let fenced = false
  const close = () => {
    if (question) faq.push({ q: question.q, a: plain(question.lines.join(' ')) })
    question = null
  }
  for (const line of md.split('\n')) {
    if (line.startsWith('```')) {
      fenced = !fenced
      continue
    }
    if (fenced) continue
    const h2 = /^## (.+)$/.exec(line)
    const h3 = /^### (.+)$/.exec(line)
    if (h2) {
      close()
      section = h2[1]
      toc.push({ id: slug(section), text: section })
    } else if (h3) {
      close()
      if (section.startsWith('Step 3')) agents.push({ id: slug(h3[1]), text: h3[1] })
      if (section === 'Questions') question = { q: h3[1], lines: [] }
    } else if (question && line.trim()) {
      question.lines.push(line)
    }
  }
  close()
  return { toc, agents, faq }
}

/** What search engines read as structured data: the article, its questions and where it sits on the site. */
export function agentsJsonLd() {
  const { faq } = parseGuide(GUIDE_MD)
  const author = { '@type': 'Person', name: AUTHOR.name, url: AUTHOR.url, sameAs: [AUTHOR.url, AUTHOR.social] }
  return {
    '@context': 'https://schema.org',
    '@graph': [
      {
        '@type': 'TechArticle',
        '@id': `${AGENTS_URL}#article`,
        headline: AGENTS_H1,
        description: AGENTS_DESCRIPTION,
        url: AGENTS_URL,
        mainEntityOfPage: AGENTS_URL,
        image: `${SITE}/shared/og.png`,
        inLanguage: 'en',
        datePublished: AGENTS_PUBLISHED,
        dateModified: AGENTS_UPDATED,
        author,
        about: { '@type': 'SoftwareApplication', name: 'Handset', url: `${SITE}/` },
        isAccessibleForFree: true,
      },
      {
        '@type': 'FAQPage',
        '@id': `${AGENTS_URL}#questions`,
        mainEntity: faq.map((f) => ({
          '@type': 'Question',
          name: f.q,
          acceptedAnswer: { '@type': 'Answer', text: f.a },
        })),
      },
      {
        '@type': 'BreadcrumbList',
        itemListElement: [
          { '@type': 'ListItem', position: 1, name: 'Handset', item: `${SITE}/` },
          { '@type': 'ListItem', position: 2, name: 'AI agents', item: AGENTS_URL },
        ],
      },
    ],
  }
}
