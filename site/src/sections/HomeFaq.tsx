import { Link } from '@tanstack/react-router'

const items: { q: string; a: string; more?: { to: string; label: string } }[] = [
  {
    q: 'Is it free?',
    a: 'Yes. Handset is free and open source under the Apache-2.0 licence.',
  },
  {
    q: 'What do I need besides Handset?',
    a: 'adb, to talk to an Android phone, and ffmpeg for recordings. Homebrew and Scoop install what they need; with a plain download the install notes say how.',
  },
  {
    q: 'Does it work with an iPhone?',
    a: 'On macOS, an iPhone or iPad can be shown over USB, with sound. That one is view only for now.',
  },
  {
    q: 'Does it run on Linux?',
    a: 'Yes, but it is experimental: it has not run on a real Linux desktop yet. Releases include a tarball for x86_64 and aarch64; elsewhere you build it from source.',
  },
  {
    q: 'Can an AI agent use my phone?',
    a: 'Yes. Handset has an MCP server, so agents such as Claude Code, Cursor, VS Code, Codex and Gemini CLI can read the screen and tap and type on your Android phone. It is off until you start Handset with --ctl, and it stays on your computer.',
    more: { to: '/agents', label: 'Read the setup guide' },
  },
  {
    q: 'Can someone watch my phone in a browser?',
    a: 'Yes. Right-click the phone and choose Share. Anyone on your network opens the link, and you are asked about each person before they see anything.',
  },
]

export function HomeFaq() {
  return (
    <section className="mx-auto max-w-3xl px-5 py-12 sm:px-8 md:py-20">
      <h2 className="display h2 text-center">Questions</h2>
      <div className="mt-8 space-y-3">
        {items.map((it) => (
          <details key={it.q} className="group rounded-2xl border-[1.5px] border-line bg-card px-5 py-4 open:border-ink">
            <summary className="display flex cursor-pointer list-none items-center justify-between gap-4 text-lg !tracking-[-0.01em] [&::-webkit-details-marker]:hidden">
              {it.q}
              <span className="grid size-7 shrink-0 place-items-center rounded-full bg-paper-2 text-xl leading-none transition-transform group-open:rotate-45" aria-hidden="true">
                +
              </span>
            </summary>
            <p className="mt-3 text-ink-2">
              {it.a}
              {it.more && (
                <>
                  {' '}
                  <Link to={it.more.to} className="font-medium text-ink underline underline-offset-4 hover:bg-marker">
                    {it.more.label}
                  </Link>
                  .
                </>
              )}
            </p>
          </details>
        ))}
      </div>
      <p className="mt-6 text-center text-ink-2">
        More fixes, like Wi-Fi pairing, are in the{' '}
        <Link to="/faq" className="font-medium text-ink underline underline-offset-4 hover:bg-marker">
          full FAQ
        </Link>
        .
      </p>
    </section>
  )
}
