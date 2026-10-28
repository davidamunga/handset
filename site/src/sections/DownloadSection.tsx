import { CopyBlock } from '../components/CopyBlock'
import { BREW, LINUX_GUIDE, LINUX_INSTALL, RELEASES, REPO, SCOOP } from '../links'
import { useReleaseInfo } from '../release'
import { useLinuxArch, type Download } from '../useRelease'

export function DownloadSection() {
  const { release, platform } = useReleaseInfo()
  const arch = useLinuxArch()
  const other = arch === 'x86_64' ? 'aarch64' : 'x86_64'
  const linux = release?.linux[arch]
  const order: ('mac' | 'windows' | 'linux')[] =
    platform === 'windows' ? ['windows', 'mac', 'linux'] : platform === 'linux' ? ['linux', 'mac', 'windows'] : ['mac', 'windows', 'linux']
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

        <div className="mt-10 grid gap-5 md:grid-cols-2 lg:grid-cols-3">
          {order.map((os) =>
            os === 'linux' ? (
              <Card
                key="linux"
                title="Linux"
                note={
                  linux
                    ? 'Experimental: it has not run on a real Linux desktop yet. A tarball for Ubuntu 22.04 or Debian 12 and newer, Fedora and Arch (and Omarchy); unpack it and put handset on your PATH. Needs ALSA. Alpine and NixOS build from source.'
                    : 'Experimental: there is no download yet, so it installs from source with Cargo. Works on Ubuntu 22.04 or Debian 12 and newer, Fedora, Arch (and Omarchy), Alpine and NixOS. It has not run on a real Linux desktop yet.'
                }
                file={linux}
                fallback={LINUX_GUIDE}
                cta={linux ? `Download for Linux (${arch})` : 'Read the Linux guide'}
                alt={release?.linux[other] && { label: `${other} instead`, url: release.linux[other].url }}
                command={LINUX_INSTALL}
                commandLabel={
                  linux
                    ? 'Or build it with Cargo (needs Rust 1.91 or newer, cmake, nasm and ALSA headers)'
                    : 'Install with Cargo (needs Rust 1.91 or newer, cmake, nasm and ALSA headers)'
                }
                featured={platform === 'linux'}
              />
            ) : os === 'mac' ? (
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
          plain download or a Linux build, the{' '}
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
  alt?: { label: string; url: string }
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
      {props.alt && (
        <a href={props.alt.url} className="ml-4 text-sm text-stage-text underline underline-offset-4 hover:text-paper">
          {props.alt.label}
        </a>
      )}
      <p className="mt-7 text-sm text-stage-text">{props.commandLabel}</p>
      <CopyBlock text={props.command} />
    </div>
  )
}
