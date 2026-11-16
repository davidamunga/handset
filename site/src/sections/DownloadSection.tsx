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
    <section id="download" className="scroll-mt-0">
      <div className="mx-auto max-w-6xl 2xl:max-w-7xl px-5 py-12 sm:px-8 md:py-20">
        <div className="flex flex-wrap items-end justify-between gap-4">
          <h2 className="display h2">
            Get <span className="hl">Handset</span>
          </h2>
          <p className="text-ink-2">
            {release ? (
              <>
                Latest: <a className="underline underline-offset-4 hover:bg-marker" href={release.page}>v{release.version}</a>
                {release.downloads.total > 0 && <> · {count(release.downloads.total)} downloads</>}
              </>
            ) : (
              <a className="underline underline-offset-4 hover:bg-marker" href={RELEASES}>
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
                downloads={release?.downloads.linux}
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
                downloads={release?.downloads.mac}
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
                downloads={release?.downloads.windows}
                fallback={release?.page ?? RELEASES}
                cta="Download for Windows"
                command={SCOOP}
                commandLabel="Or with Scoop, which also installs adb"
                featured={platform === 'windows'}
              />
            ),
          )}
        </div>

        <p className="mt-8 max-w-3xl text-[0.95rem] text-ink-2">
          Handset needs <code className="rounded-md border border-line bg-paper-2 px-1.5 py-0.5 font-mono text-[0.8rem] text-ink">adb</code> to talk to an Android phone, and{' '}
          <code className="rounded-md border border-line bg-paper-2 px-1.5 py-0.5 font-mono text-[0.8rem] text-ink">ffmpeg</code> for recordings. Homebrew and Scoop install what they need. With a
          plain download or a Linux build, the{' '}
          <a className="underline underline-offset-4 hover:bg-marker" href={`${REPO}#install`}>
            install notes
          </a>{' '}
          say how. Each release lists a checksum and a signature so you can check the file.
        </p>
      </div>
    </section>
  )
}

const count = (n: number) => n.toLocaleString('en-US')

function Card(props: {
  title: string
  note: string
  file?: Download
  downloads?: number
  fallback: string
  cta: string
  alt?: { label: string; url: string }
  command: string
  commandLabel: string
  featured: boolean
}) {
  return (
    <div
      className={`card min-w-0 p-7 text-ink sm:p-8 ${props.featured ? '!border-ink' : ''}`}
    >
      <div className="flex flex-wrap items-baseline justify-between gap-x-4 gap-y-1">
        <h3 className="display text-3xl">{props.title}</h3>
        {props.file && (
          <span className="label text-ink-2">
            {props.file.size}
            {props.downloads ? ` · ${count(props.downloads)} downloads` : ''}
          </span>
        )}
      </div>
      <p className="mt-3 text-[0.95rem] text-ink-2">{props.note}</p>
      <a href={props.file?.url ?? props.fallback} className={`btn mt-6 ${props.featured ? 'btn-green' : ''}`}>
        {props.cta}
      </a>
      {props.alt && (
        <a href={props.alt.url} className="ml-4 text-sm underline underline-offset-4 hover:bg-marker">
          {props.alt.label}
        </a>
      )}
      <p className="mt-7 text-sm text-ink-2">{props.commandLabel}</p>
      <CopyBlock text={props.command} />
    </div>
  )
}
