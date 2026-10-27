import { CopyBlock } from '../components/CopyBlock'
import { BREW, RELEASES, REPO, SCOOP } from '../links'
import { useReleaseInfo } from '../release'
import type { Download } from '../useRelease'

export function DownloadSection() {
  const { release, platform } = useReleaseInfo()
  const order: ('mac' | 'windows')[] = platform === 'windows' ? ['windows', 'mac'] : ['mac', 'windows']
  return (
    <section id="download" className="scroll-mt-0 bg-stage text-paper">
      <div className="mx-auto max-w-6xl px-5 py-16 sm:px-8 md:py-24">
        <div className="flex flex-wrap items-end justify-between gap-4">
          <h2 className="font-display text-[clamp(2rem,4.5vw,3.25rem)] font-bold leading-none tracking-[-0.03em]">
            Get Handset
          </h2>
          <p className="text-stage-text">
            {release ? (
              <>
                Latest: <a className="underline underline-offset-4 hover:text-paper" href={release.page}>v{release.version}</a>
              </>
            ) : (
              <a className="underline underline-offset-4 hover:text-paper" href={RELEASES}>
                See the latest release
              </a>
            )}
          </p>
        </div>

        <div className="mt-10 grid gap-5 md:grid-cols-2">
          {order.map((os) =>
            os === 'mac' ? (
              <Card
                key="mac"
                title="macOS"
                note="macOS 12 or later, Apple Silicon and Intel. Signed and notarised."
                file={release?.mac}
                fallback={release?.page ?? RELEASES}
                cta="Download for macOS"
                command={BREW}
                commandLabel="Or with Homebrew, which also installs adb and ffmpeg"
                featured={platform === 'mac'}
              />
            ) : (
              <Card
                key="windows"
                title="Windows"
                note="Windows 10 or 11. Not code-signed yet, so SmartScreen asks once: More info, then Run anyway."
                file={release?.windows}
                fallback={release?.page ?? RELEASES}
                cta="Download for Windows"
                command={SCOOP}
                commandLabel="Or with Scoop, which also installs adb"
                featured={platform === 'windows'}
              />
            ),
          )}
        </div>

        <p className="mt-8 max-w-3xl text-[0.95rem] text-stage-text">
          Handset needs <code className="rounded bg-stage-2 px-1.5 py-0.5 text-paper">adb</code> to talk to an Android phone, and{' '}
          <code className="rounded bg-stage-2 px-1.5 py-0.5 text-paper">ffmpeg</code> for recordings. Homebrew and Scoop install what they need. With a
          plain download, the{' '}
          <a className="underline underline-offset-4 hover:text-paper" href={`${REPO}#install`}>
            install notes
          </a>{' '}
          say how. Each release lists a checksum and a signature so you can check the file.
        </p>
      </div>
    </section>
  )
}

function Card(props: {
  title: string
  note: string
  file?: Download
  fallback: string
  cta: string
  command: string
  commandLabel: string
  featured: boolean
}) {
  return (
    <div className={`min-w-0 rounded-3xl p-7 sm:p-8 ${props.featured ? 'bg-stage-2 ring-1 ring-stage-line' : 'bg-stage ring-1 ring-stage-line'}`}>
      <div className="flex items-baseline justify-between gap-4">
        <h3 className="font-display text-2xl font-bold">{props.title}</h3>
        {props.file && <span className="text-sm text-stage-text">{props.file.size}</span>}
      </div>
      <p className="mt-3 text-[0.95rem] text-stage-text">{props.note}</p>
      <a
        href={props.file?.url ?? props.fallback}
        className="mt-6 inline-block rounded-xl bg-paper px-5 py-3 font-semibold text-ink transition-transform hover:-translate-y-px active:translate-y-0"
      >
        {props.cta}
      </a>
      <p className="mt-7 text-sm text-stage-text">{props.commandLabel}</p>
      <CopyBlock text={props.command} />
    </div>
  )
}
