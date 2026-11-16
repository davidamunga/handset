import { Children, isValidElement, useMemo, type ReactNode } from 'react'
import Markdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import { GUIDE_MD, parseGuide, slug } from '../content/agents'
import { REPO } from '../links'
import { CopyBlock } from './CopyBlock'

/** The words inside a heading, as one string. */
function words(children: ReactNode): string {
  return Children.toArray(children)
    .map((c) => (typeof c === 'string' || typeof c === 'number' ? String(c) : isValidElement<{ children?: ReactNode }>(c) ? words(c.props.children) : ''))
    .join('')
}

/**
 * The guide to using an AI agent with Handset's MCP server. It uses plain links, not the router's, because the
 * build renders it to HTML without a router (so search engines and AI crawlers can read it without running
 * scripts), and the page then takes over from that copy.
 */
export function AgentsGuide() {
  const { toc, agents } = useMemo(() => parseGuide(GUIDE_MD), [])
  return (
    <article className="mx-auto max-w-6xl 2xl:max-w-7xl px-5 pb-24 pt-10 sm:px-8 md:pt-16">
      <header className="grid items-center gap-10 md:grid-cols-12">
        <div className="md:col-span-7">
          <p className="label inline-block rounded-full bg-marker px-3 py-1">Guide</p>
          <h1 className="display mt-5 text-[clamp(2.4rem,6vw,4.4rem)]">
            Let AI agents use your <span className="hl">Android phone</span>
          </h1>
          <p className="mt-6 max-w-xl text-xl leading-snug text-ink-2">
            Handset&apos;s MCP server gives Claude Code, Cursor, VS Code, Codex, Gemini CLI and any other MCP agent a real
            phone to read, tap and test. Free, open source, and off until you switch it on.
          </p>
          <p className="mt-8 flex flex-wrap gap-3">
            <a href="#step-3-add-it-to-your-agent" className="btn">
              Pick your agent
            </a>
            <a href="/#download" className="btn btn-light">
              Download Handset
            </a>
          </p>
          <ul className="label mt-8 flex flex-wrap gap-x-5 gap-y-2 text-ink-2">
            <li>Free &amp; open source</li>
            <li>{agents.length - 1} agents covered</li>
            <li>Android</li>
            <li>One command to add</li>
          </ul>
        </div>
        <div className="md:col-span-5">
          <div className="rounded-[2rem] bg-ink px-6 py-8">
            <img
              src="/shared/mirroring.png"
              alt="An Android phone shown in Handset inside a device frame, with its toolbar beside it: the phone an AI agent reads and taps"
              width={880}
              height={1550}
              className="mx-auto h-[min(26rem,60vh)] w-auto"
            />
          </div>
        </div>
      </header>

      <nav aria-label="Jump to your agent" className="mt-12">
        <p className="label text-ink-2">Jump to your agent</p>
        <ul className="mt-3 flex flex-wrap gap-2">
          {agents.map((a) => (
            <li key={a.id}>
              <a href={`#${a.id}`} className="inline-block rounded-full border-[1.5px] border-line bg-card px-4 py-1.5 text-[0.95rem] hover:border-ink hover:bg-marker">
                {a.text}
              </a>
            </li>
          ))}
        </ul>
      </nav>

      <div className="mt-14 grid gap-10 lg:grid-cols-12">
        <aside className="hidden lg:col-span-3 lg:block">
          <nav aria-label="On this page" className="sticky top-6">
            <p className="label text-ink-2">On this page</p>
            <ol className="mt-3 space-y-1.5 text-[0.95rem]">
              {toc.map((h) => (
                <li key={h.id}>
                  <a href={`#${h.id}`} className="block rounded-lg px-2 py-1 text-ink-2 hover:bg-paper-2 hover:text-ink">
                    {h.text}
                  </a>
                </li>
              ))}
            </ol>
          </nav>
        </aside>
        <div className="legal lg:col-span-9 min-w-0 max-w-3xl">
          <Markdown
            remarkPlugins={[remarkGfm]}
            components={{
              h2: ({ children }) => {
                const id = slug(words(children))
                return (
                  <h2 id={id} className="scroll-mt-6 border-t border-line pt-8">
                    <a href={`#${id}`} className="no-underline">
                      {children}
                    </a>
                  </h2>
                )
              },
              h3: ({ children }) => <h3 id={slug(words(children))} className="scroll-mt-6">{children}</h3>,
              // a fenced block is a command or a settings file to copy
              pre: ({ children }) => {
                const code = Children.toArray(children)[0]
                const text = isValidElement<{ children?: ReactNode }>(code) ? words(code.props.children) : ''
                return <CopyBlock text={text.replace(/\n$/, '')} />
              },
              table: ({ children }) => (
                <div className="mt-4 overflow-x-auto">
                  <table>{children}</table>
                </div>
              ),
              a: ({ href, children }) => (
                <a href={href} rel={href?.startsWith('http') ? 'noreferrer' : undefined}>
                  {children}
                </a>
              ),
            }}
          >
            {GUIDE_MD}
          </Markdown>
          <p>
            Something missing, or a client that needs different settings? Open an issue on{' '}
            <a href={`${REPO}/issues`}>GitHub</a>. The full reference for the commands behind these tools is in the{' '}
            <a href={`${REPO}/blob/main/docs/agents.md`}>agents guide in the repository</a>.
          </p>
        </div>
      </div>
    </article>
  )
}
