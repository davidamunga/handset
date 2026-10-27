import { useState } from 'react'
import { Mark } from './Mark'
import { AUTHOR, BREW, RELEASES, REPO, SCOOP } from './links'
import { detectPlatform, useRelease, type Download, type Platform, type Release } from './useRelease'

const img = (name: string) => `/shared/${name}`

export function App() {
  const release = useRelease()
  const platform = detectPlatform()
  return (
    <>
      <a
        href="#main"
        className="sr-only focus:not-sr-only focus:fixed focus:left-4 focus:top-4 focus:z-50 focus:rounded-lg focus:bg-ink focus:px-4 focus:py-2 focus:text-paper"
      >
        Skip to content
      </a>
      <Header release={release} />
      <main id="main">
        <Hero release={release} platform={platform} />
        <Features />
        <Connect />
        <DownloadSection release={release} platform={platform} />
        <About />
      </main>
      <Footer />
    </>
  )
}

/* ---------------------------------------------------------------- header */

function Header({ release }: { release: Release | null }) {
  return (
    <header className="mx-auto flex max-w-6xl items-center justify-between px-5 py-5 sm:px-8">
      <a href="#top" className="flex items-center gap-2.5 font-display text-xl font-bold tracking-tight" id="top">
        <Mark className="size-9" />
        Handset
      </a>
      <nav aria-label="Main" className="flex items-center gap-1 text-[0.95rem] sm:gap-2">
        <a className="hidden rounded-lg px-3 py-2 hover:bg-paper-2 sm:block" href="#features">
          What it does
        </a>
        <a className="hidden rounded-lg px-3 py-2 hover:bg-paper-2 sm:block" href="#download">
          Download
        </a>
        <a className="rounded-lg px-3 py-2 hover:bg-paper-2" href={REPO}>
          GitHub
        </a>
        <a
          href={release?.page ?? RELEASES}
          className="ml-1 rounded-xl bg-ink px-4 py-2 font-medium text-paper transition-transform hover:-translate-y-px active:translate-y-0"
        >
          {release ? `v${release.version}` : 'Releases'}
        </a>
      </nav>
    </header>
  )
}

/* ------------------------------------------------------------------ hero */

function Hero({ release, platform }: { release: Release | null; platform: Platform }) {
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
          Handset mirrors and controls your Android phone from a Mac or a Windows PC, inside a real device frame. Capture
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
          Free and open source, Apache-2.0. macOS 12 or later, and Windows 10 or 11.
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
  return { label: 'Download', url: '#download' }
}

/* -------------------------------------------------------------- features */

function Features() {
  return (
    <section id="features" className="mx-auto max-w-6xl scroll-mt-4 px-5 py-8 sm:px-8 md:py-16">
      <h2 className="font-display text-[clamp(2rem,4.5vw,3.25rem)] font-bold leading-none tracking-[-0.03em]">
        What you can do with it
      </h2>

      <Row
        number="01"
        title="Use your phone from the desktop"
        body="Live video and device audio over adb. Click to touch, scroll, type on your keyboard, rotate to landscape, and copy and paste between the two. Plug in a cable, or use Wi-Fi."
        points={['Touch, scroll, keyboard and text input', 'Clipboard shared both ways', 'Starts when you plug the phone in']}
        stage
      >
        <img
          src={img('windows-mirroring.png')}
          alt="Handset mirroring an Android phone on Windows, inside a dark device frame"
          width={636}
          height={1280}
          loading="lazy"
          className="mx-auto h-[min(34rem,70vh)] w-auto"
        />
      </Row>

      <Row
        number="02"
        title="Inside a real device frame"
        body="Twenty-one frames, from punch-hole and notch to foldable, flip and tablet, or bring your own PNG. The window is transparent, so the phone sits on your desktop like an object, not a rectangle."
        points={['Screenshots as transparent PNGs', 'Recordings as MP4 with sound, or GIF', 'Switch frames while it runs']}
        flip
      >
        <img
          src={img('styled-example.png')}
          alt="A framed phone on a green and purple wallpaper, 16 by 9"
          width={1000}
          height={562}
          loading="lazy"
          className="w-full rounded-3xl border border-line"
        />
      </Row>

      <Row
        number="03"
        title="Style every capture"
        body="Put a screenshot or a recording on a gradient, a wallpaper, a colour or your own picture. Set the padding, shadow and corners, pick an aspect ratio from 1:1 to 16:9, and trim a recording while you watch it. Save the look as a preset. Your original is never touched."
        points={['Presets with thumbnails of your own picture', 'Light and dark, following your system', 'Export a PNG, MP4 or GIF']}
      >
        <img
          src={img('editor.png')}
          alt="The Handset style editor: presets, background, layout and aspect ratio controls beside a live preview"
          width={1280}
          height={933}
          loading="lazy"
          className="w-full rounded-2xl border border-line"
        />
      </Row>
    </section>
  )
}

function Row(props: {
  number: string
  title: string
  body: string
  points: string[]
  children: React.ReactNode
  stage?: boolean
  flip?: boolean
}) {
  return (
    <article className="mt-16 grid items-center gap-8 md:mt-24 md:grid-cols-12 md:gap-12">
      <div className={`md:col-span-5 ${props.flip ? 'md:order-2' : ''}`}>
        <p className="font-display text-sm font-bold tracking-widest text-ink-2">{props.number}</p>
        <h3 className="mt-3 font-display text-[clamp(1.6rem,3vw,2.2rem)] font-bold leading-[1.05] tracking-[-0.02em]">
          {props.title}
        </h3>
        <p className="mt-4 text-ink-2">{props.body}</p>
        <ul className="mt-5 space-y-2">
          {props.points.map((p) => (
            <li key={p} className="flex gap-3">
              <span className="mt-[0.7em] size-1.5 shrink-0 rounded-full bg-ink" aria-hidden="true" />
              {p}
            </li>
          ))}
        </ul>
      </div>
      <div className={`md:col-span-7 ${props.flip ? 'md:order-1' : ''}`}>
        {/* the screenshot has its own near-black backdrop, so the stage panel matches it */}
        {props.stage ? <div className="rounded-[2rem] bg-[#100f14] px-6 py-10 sm:py-12">{props.children}</div> : props.children}
      </div>
    </article>
  )
}

/* --------------------------------------------------------------- connect */

function Connect() {
  const shots = [
    { src: 'connect-home.png', title: 'Cable', text: 'Plug in and the steps update as you go. The logo looks around while it waits.' },
    { src: 'connect-wifi.png', title: 'Wi-Fi', text: 'Phones that are paired on your network show up to tap, or type an address.' },
    { src: 'connect-qr.png', title: 'QR code', text: 'Scan a code with the phone to pair over Wi-Fi, no cable at all.' },
  ]
  return (
    <section className="bg-paper-2 py-16 md:py-24">
      <div className="mx-auto max-w-6xl px-5 sm:px-8">
        <div className="max-w-2xl">
          <h2 className="font-display text-[clamp(2rem,4.5vw,3.25rem)] font-bold leading-none tracking-[-0.03em]">
            Connect any way you like
          </h2>
          <p className="mt-4 text-ink-2">
            With no phone connected, Handset opens on a connect screen inside the frame. It also shows an iPhone or iPad
            on macOS, with sound. That one is view only for now.
          </p>
        </div>
        <div className="mt-12 grid gap-10 sm:grid-cols-3 sm:gap-6">
          {shots.map((s) => (
            <figure key={s.src} className="mx-auto max-w-[17rem] sm:max-w-none">
              <img src={img(s.src)} alt={`The ${s.title} connect screen`} width={529} height={1000} loading="lazy" className="mx-auto w-full max-w-[15rem]" />
              <figcaption className="mt-4 text-center">
                <span className="font-display text-lg font-bold">{s.title}</span>
                <span className="mx-auto mt-1 block max-w-[16rem] text-[0.95rem] text-ink-2">{s.text}</span>
              </figcaption>
            </figure>
          ))}
        </div>
      </div>
    </section>
  )
}

/* -------------------------------------------------------------- download */

function DownloadSection({ release, platform }: { release: Release | null; platform: Platform }) {
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

function CopyBlock({ text }: { text: string }) {
  const [copied, setCopied] = useState(false)
  const copy = () => {
    navigator.clipboard?.writeText(text).then(() => {
      setCopied(true)
      setTimeout(() => setCopied(false), 1600)
    })
  }
  return (
    <div className="mt-2 flex min-w-0 items-start justify-between gap-3 rounded-xl bg-black/40 py-3 pl-4 pr-3 ring-1 ring-stage-line">
      <pre className="min-w-0 flex-1 overflow-x-auto whitespace-pre text-sm leading-relaxed text-paper">{text}</pre>
      <button
        type="button"
        onClick={copy}
        className="shrink-0 rounded-lg px-2.5 py-1 text-sm font-medium text-stage-text transition-colors hover:bg-stage-2 hover:text-paper"
        aria-live="polite"
      >
        {copied ? 'Copied' : 'Copy'}
      </button>
    </div>
  )
}

/* ----------------------------------------------------------------- about */

function About() {
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
            and is licensed under{' '}
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

/* ---------------------------------------------------------------- footer */

function Footer() {
  return (
    <footer className="bg-stage text-stage-text">
      <div className="mx-auto max-w-6xl px-5 py-12 sm:px-8">
        <div className="flex flex-wrap items-center justify-between gap-6">
          <div className="flex items-center gap-2.5 font-display text-xl font-bold text-paper">
            <Mark className="size-9" />
            Handset
          </div>
          <nav aria-label="Footer" className="flex flex-wrap gap-x-6 gap-y-2 text-[0.95rem]">
            <a className="hover:text-paper" href={REPO}>GitHub</a>
            <a className="hover:text-paper" href={`${REPO}/releases`}>Releases</a>
            <a className="hover:text-paper" href={`${REPO}/blob/main/CHANGELOG.md`}>Changelog</a>
            <a className="hover:text-paper" href={`${REPO}/issues`}>Report a problem</a>
            <a className="hover:text-paper" href={`${REPO}/blob/main/SECURITY.md`}>Security</a>
            <a className="hover:text-paper" href={`${REPO}/blob/main/NOTICE`}>Notices</a>
          </nav>
        </div>
        <p className="mt-8 max-w-3xl text-sm leading-relaxed">
          © 2026 {AUTHOR.name}. Handset is Apache-2.0 licensed. scrcpy is © Genymobile and Romain Vimont. Android is a
          trademark of Google LLC. iPhone, iPad and macOS are trademarks of Apple Inc. Windows is a trademark of
          Microsoft. They are named here only to say what Handset works with, and none of those companies is affiliated
          with Handset.
        </p>
      </div>
    </footer>
  )
}
