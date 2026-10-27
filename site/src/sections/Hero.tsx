import { RELEASES, REPO } from '../links'
import { useReleaseInfo } from '../release'
import type { Platform, Release } from '../useRelease'

const img = (name: string) => `/shared/${name}`

export function Hero() {
  const { release, platform } = useReleaseInfo()
  const primary = primaryDownload(release, platform)
  return (
    <section className="mx-auto grid max-w-6xl items-center gap-12 px-5 pb-20 pt-8 sm:px-8 md:grid-cols-[1.15fr_1fr] md:gap-8 md:pb-28 md:pt-14">
      <div>
        <h1
          className="rise font-display text-[clamp(3rem,8vw,5.75rem)] font-extrabold leading-[0.94] tracking-[-0.035em]"
          style={{ ['--i' as string]: 0 }}
        >
          Your phone,
          <br />
          on your desktop.
        </h1>
        <div className="rise mt-7 flex items-center gap-2" style={{ ['--i' as string]: 1 }} aria-hidden="true">
          <span className="h-1.5 w-14 rounded-full bg-android" />
          <span className="h-1.5 w-14 rounded-full bg-ios" />
        </div>
        <p className="rise mt-7 max-w-[34rem] text-xl leading-snug text-ink-2" style={{ ['--i' as string]: 2 }}>
          Handset mirrors and controls your Android phone from a Mac, a Windows PC or Linux, inside a real device frame. Capture
          it, style it, share it.
        </p>
        <div className="rise mt-9 flex flex-wrap items-center gap-3" style={{ ['--i' as string]: 3 }}>
          <a
            href={primary.url}
            className="rounded-xl bg-ink px-6 py-3.5 text-lg font-semibold text-paper transition-transform hover:-translate-y-px active:translate-y-0"
          >
            {primary.label}
          </a>
          <a
            href={REPO}
            className="rounded-xl border border-line px-6 py-3.5 text-lg font-semibold transition-colors hover:bg-paper-2"
          >
            View on GitHub
          </a>
        </div>
        <p className="rise mt-5 text-sm text-ink-2" style={{ ['--i' as string]: 4 }}>
          Free and open source, Apache-2.0. macOS 12 or later, Windows 10 or 11, and Linux (experimental).
        </p>
      </div>

      <div className="relative mx-auto w-[min(78vw,21rem)] md:w-full md:max-w-[22rem]" aria-hidden="true">
        <div className="echo-left absolute left-[-4%] top-[9%] aspect-[80/128] w-[68%] rounded-[2.2rem] bg-android p-[3.5%]">
          <div className="size-full rounded-[1.7rem] bg-android-deep" />
        </div>
        <div className="echo-right absolute right-[-4%] top-[1%] aspect-[80/128] w-[68%] rounded-[2.2rem] bg-ios p-[3.5%]">
          <div className="size-full rounded-[1.7rem] bg-ios-deep" />
        </div>
        <img
          src={img('connect-home.png')}
          alt=""
          width={529}
          height={1000}
          className="rise relative"
          style={{ ['--i' as string]: 3 }}
        />
      </div>
    </section>
  )
}

function primaryDownload(release: Release | null, platform: Platform): { label: string; url: string } {
  const fall = release?.page ?? RELEASES
  if (platform === 'windows') return { label: 'Download for Windows', url: release?.windows?.url ?? fall }
  if (platform === 'mac') return { label: 'Download for macOS', url: release?.mac?.url ?? fall }
  if (platform === 'linux') return { label: 'Get it for Linux', url: '#download' }
  return { label: 'Download', url: '#download' }
}
