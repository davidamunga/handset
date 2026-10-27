import { createFileRoute } from '@tanstack/react-router'
import Markdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import changelog from '../../../CHANGELOG.md?raw'
import { LegalPage } from '../components/LegalPage'
import { REPO } from '../links'

export const Route = createFileRoute('/changelog')({
  head: () => ({
    meta: [
      { title: 'Changelog | Handset' },
      { name: 'description', content: 'What changed in each release of Handset, newest first.' },
    ],
  }),
  component: Changelog,
})

const slug = (children: unknown) =>
  String(Array.isArray(children) ? children.join('') : children)
    .toLowerCase()
    .replace(/[^a-z0-9.]+/g, '-')
    .replace(/^-|-$/g, '')

// the file starts with its own "# Changelog" heading, which the page already shows as its title
const body = changelog.replace(/^# Changelog\s*/, '')

function Changelog() {
  return (
    <LegalPage
      title="Changelog"
      updated="the latest release"
      intro="What changed in each release of Handset, newest first. This is the project's CHANGELOG.md."
      stamp={false}
    >
      <Markdown
        remarkPlugins={[remarkGfm]}
        components={{
          h2: ({ children }) => (
            <h2 id={slug(children)} className="scroll-mt-6 border-t border-line pt-8">
              <a href={`#${slug(children)}`} className="no-underline">
                {children}
              </a>
            </h2>
          ),
          h3: ({ children }) => <h3 className="mt-6 font-display text-lg font-bold">{children}</h3>,
          a: ({ href, children }) => (
            <a href={href} rel={href?.startsWith('http') ? 'noreferrer' : undefined}>
              {children}
            </a>
          ),
        }}
      >
        {body}
      </Markdown>
      <p>
        The same list is on{' '}
        <a href={`${REPO}/releases`}>GitHub releases</a>, with each release&apos;s files.
      </p>
    </LegalPage>
  )
}
