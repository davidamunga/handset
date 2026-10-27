import { AUTHOR, REPO } from '../links'

export function About() {
  return (
    <section id="about" className="mx-auto max-w-6xl px-5 py-16 sm:px-8 md:py-24">
      <div className="grid gap-10 md:grid-cols-12">
        <h2 className="font-display text-[clamp(2rem,4.5vw,3.25rem)] font-bold leading-none tracking-[-0.03em] md:col-span-5">
          Open source, built on scrcpy
        </h2>
        <div className="space-y-5 text-ink-2 md:col-span-7">
          <p>
            Handset is made by{' '}
            <a className="font-medium text-ink underline underline-offset-4" href={AUTHOR.url}>
              {AUTHOR.name}
            </a>{' '}
            (
            <a className="font-medium text-ink underline underline-offset-4" href={AUTHOR.social}>
              {AUTHOR.handle}
            </a>
            ) and is licensed under{' '}
            <a className="font-medium text-ink underline underline-offset-4" href={`${REPO}/blob/main/LICENSE`}>
              Apache-2.0
            </a>
            . Read the code, report a problem, or send a fix on{' '}
            <a className="font-medium text-ink underline underline-offset-4" href={REPO}>
              GitHub
            </a>
            .
          </p>
          <p>
            It speaks the{' '}
            <a className="font-medium text-ink underline underline-offset-4" href="https://github.com/Genymobile/scrcpy">
              scrcpy
            </a>{' '}
            protocol and embeds the unmodified scrcpy server by Genymobile and Romain Vimont, also Apache-2.0, to read the
            phone. Handset is an independent app. It is not affiliated with or endorsed by Genymobile.
          </p>
          <p>
            Text is set in{' '}
            <a className="font-medium text-ink underline underline-offset-4" href="https://rsms.me/inter/">
              Inter
            </a>{' '}
            by the Inter Project Authors, under the SIL Open Font License 1.1, and in Bricolage Grotesque by Mathieu Triay,
            under the same licence.
          </p>
        </div>
      </div>
    </section>
  )
}
