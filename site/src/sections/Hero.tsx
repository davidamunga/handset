import { useState } from 'react'
import { RELEASES, REPO } from '../links'
import { useReleaseInfo } from '../release'
import type { Release } from '../useRelease'

type Os = 'mac' | 'windows' | 'linux'
const SYSTEMS: { os: Os; word: string }[] = [
  { os: 'mac', word: 'Mac' },
  { os: 'windows', word: 'PC' },
  { os: 'linux', word: 'Linux PC' },
]
const DEVICES = ['phone', 'Pixel', 'Galaxy', 'foldable']

const img = (name: string) => `/shared/${name}`

/** A highlighted word that changes to the next one when you tap it. */
function Swap({ words, i, next, label }: { words: string[]; i: number; next: () => void; label: string }) {
  return (
    <button type="button" className="hl" onClick={next} aria-label={`${label}: ${words[i]}. Tap to change.`}>
      {words[i]}
    </button>
  )
}

/** A small round label stuck on a picture. */
function Chip({ children, className = '' }: { children: string; className?: string }) {
  return <span className={`label absolute z-20 rounded-full bg-marker px-3 py-1.5 text-ink ${className}`}>{children}</span>
}

export function Hero() {
  const { release, platform } = useReleaseInfo()
  const [sys, setSys] = useState(() => Math.max(0, SYSTEMS.findIndex((s) => s.os === platform)))
  const [dev, setDev] = useState(0)
  const cycle = (n: number, set: (f: (v: number) => number) => void) => () => set((v) => (v + 1) % n)
  const primary = primaryDownload(release, SYSTEMS[sys].os)
  return (
    <section className="mx-auto grid max-w-6xl 2xl:max-w-7xl items-center gap-8 px-5 pb-12 pt-2 sm:px-8 lg:grid-cols-[0.8fr_1.2fr] lg:gap-4 lg:pb-16">
      <div className="@container text-center lg:text-left">
        {/* sized to its column, one unbroken line each, so a longer swapped word never wraps its pill */}
        <h1 className="display rise whitespace-nowrap" style={{ fontSize: 'clamp(1.9rem, 10cqw, 4.8rem)' }}>
          Your <Swap words={DEVICES} i={dev} next={cycle(DEVICES.length, setDev)} label="Device" />
          <br />
          on your <span className="whitespace-nowrap"><Swap words={SYSTEMS.map((s) => s.word)} i={sys} next={cycle(SYSTEMS.length, setSys)} label="Computer" />.</span>
        </h1>
        <div className="rise mt-7 flex flex-wrap justify-center gap-3 lg:justify-start" style={{ ['--i' as string]: 1 }}>
          <a href={primary.url} className="btn text-lg">
            {primary.label}
          </a>
          <a href={REPO} className="btn btn-light text-lg">
            GitHub
          </a>
        </div>
        <p className="label rise mt-6 text-fade" style={{ ['--i' as string]: 2 }}>
          Free · open source
        </p>
      </div>

      {/* the app itself: the phone in its frame, with the style editor and the share page behind it */}
      <div className="relative mx-auto aspect-[1.3] w-full max-w-[46rem] 2xl:max-w-[52rem]">
        <figure className="rise absolute left-0 top-[4%] w-[47%] -rotate-3" style={{ ['--i' as string]: 3 }}>
          <img
            src={img('editor.png')}
            alt="The Handset style editor: presets, background and layout controls beside a live preview"
            width={1280}
            height={933}
            className="card w-full"
          />
          <Chip className="-bottom-3 left-4">Style</Chip>
        </figure>
        <figure className="rise absolute bottom-[3%] right-0 w-[45%] rotate-2" style={{ ['--i' as string]: 4 }}>
          <img
            src={img('share-page.png')}
            alt="The page Share shows in a browser: the phone in its frame with its keys beside it"
            width={1440}
            height={900}
            className="card w-full"
          />
          <Chip className="-top-3 right-4">Share</Chip>
        </figure>
        <figure className="rise absolute inset-y-0 left-1/2 z-10 aspect-[880/1550] h-full -translate-x-1/2" style={{ ['--i' as string]: 2 }}>
          <img
            src={img('mirroring.png')}
            alt="Handset mirroring an Android phone inside a device frame, with its toolbar beside it"
            width={880}
            height={1550}
            className="h-full w-full"
          />
          <Chip className="left-0 top-[8%]">Mirror</Chip>
        </figure>
      </div>
    </section>
  )
}

function primaryDownload(release: Release | null, os: Os): { label: string; url: string } {
  const fall = release?.page ?? RELEASES
  if (os === 'windows') return { label: 'Download for Windows', url: release?.windows?.url ?? fall }
  if (os === 'mac') return { label: 'Download for macOS', url: release?.mac?.url ?? fall }
  return { label: 'Get it for Linux', url: '#download' }
}
