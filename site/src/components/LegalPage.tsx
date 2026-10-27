import type { ReactNode } from 'react'

/** The frame of a long-form page: a title, when it last changed, and the text. */
export function LegalPage({
  title,
  updated,
  intro,
  stamp = true,
  children,
}: {
  title: string
  updated: string
  intro: string
  stamp?: boolean
  children: ReactNode
}) {
  return (
    <article className="mx-auto max-w-3xl px-5 pb-24 pt-10 sm:px-8 md:pt-16">
      <h1 className="font-display text-[clamp(2.4rem,6vw,4rem)] font-extrabold leading-none tracking-[-0.03em]">{title}</h1>
      {stamp && <p className="mt-4 text-sm text-ink-2">Last updated {updated}</p>}
      <p className="mt-6 text-xl leading-snug text-ink-2">{intro}</p>
      <div className="legal">{children}</div>
    </article>
  )
}
