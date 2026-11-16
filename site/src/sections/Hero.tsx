import { RELEASES, REPO } from '../links'
import { useReleaseInfo } from '../release'
import type { Platform, Release } from '../useRelease'

const img = (name: string) => `/shared/${name}`

export function Hero() {
  const { release, platform } = useReleaseInfo()
  const primary = primaryDownload(release, platform)
  return (
    <section className="mx-auto grid max-w-6xl items-center gap-12 px-5 pb-16 pt-10 sm:px-8 md:grid-cols-[1.2fr_1fr] md:gap-8 md:pb-24 md:pt-16">
      <div>
        <p className="label rise" style={{ ['--i' as string]: 0 }}>
          Android on macOS · Windows · Linux
        </p>
        <h1
          className="display rise mt-5 text-[clamp(2.6rem,6.2vw,4.9rem)]"
          style={{ ['--i' as string]: 1 }}
        >
          Your phone,
          <br />
          on your <span className="marker">desktop.</span>
        </h1>
        <p className="rise mt-7 max-w-[34rem] text-xl leading-snug" style={{ ['--i' as string]: 2 }}>
          Handset mirrors and controls your Android phone from a Mac, a Windows PC or Linux, inside a real device frame. Capture
          it, style it, share it.
        </p>
        <div className="rise mt-9 flex flex-wrap items-center gap-4" style={{ ['--i' as string]: 3 }}>
          <a href={primary.url} className="btn btn-green text-lg">
            {primary.label}
          </a>
          <a href={REPO} className="btn text-lg">
            View on GitHub
          </a>
        </div>
        <p className="label rise mt-7 max-w-[34rem] leading-relaxed text-ink-2" style={{ ['--i' as string]: 4 }}>
          Free and open source · Apache-2.0 · macOS 12+ · Windows 10 or 11 · Linux (experimental)
        </p>
      </div>

      <div className="relative mx-auto w-[min(78vw,21rem)] md:w-full md:max-w-[22rem]" aria-hidden="true">
        <div className="echo-left absolute left-[-4%] top-[9%] aspect-[80/128] w-[68%] rounded-[2.2rem] border-2 border-ink bg-android p-[3.5%]">
          <div className="size-full rounded-[1.7rem] bg-android-deep" />
        </div>
        <div className="echo-right absolute right-[-4%] top-[1%] aspect-[80/128] w-[68%] rounded-[2.2rem] border-2 border-ink bg-ios p-[3.5%]">
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
