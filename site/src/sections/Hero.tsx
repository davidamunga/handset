import { useState } from 'react'
import { FrameRow } from '../components/FrameRow'
import { RELEASES, REPO } from '../links'
import { useReleaseInfo } from '../release'
import type { Release } from '../useRelease'

type Os = 'mac' | 'windows' | 'linux'
const SYSTEMS: { os: Os; word: string }[] = [
  { os: 'mac', word: 'Mac' },
  { os: 'windows', word: 'Windows PC' },
  { os: 'linux', word: 'Linux desktop' },
]
const DEVICES = ['phone', 'Pixel', 'Galaxy', 'foldable']
const ACTIONS = ['capture', 'record', 'style', 'share']

/** A highlighted word that changes to the next one when you tap it. */
function Swap({ words, i, next, label }: { words: string[]; i: number; next: () => void; label: string }) {
  return (
    <button type="button" className="hl" onClick={next} aria-label={`${label}: ${words[i]}. Tap to change.`}>
      {words[i]}
    </button>
  )
}

export function Hero() {
  const { release, platform } = useReleaseInfo()
  const [sys, setSys] = useState(() => Math.max(0, SYSTEMS.findIndex((s) => s.os === platform)))
  const [dev, setDev] = useState(0)
  const [act, setAct] = useState(0)
  const cycle = (n: number, set: (f: (v: number) => number) => void) => () => set((v) => (v + 1) % n)
  const primary = primaryDownload(release, SYSTEMS[sys].os)
  return (
    <section className="mx-auto max-w-5xl px-5 pb-16 pt-4 text-center sm:px-8 md:pb-24">
      <FrameRow />
      <h1 className="display mt-10 text-[clamp(2.4rem,6.6vw,4.9rem)]">
        Your <Swap words={DEVICES} i={dev} next={cycle(DEVICES.length, setDev)} label="Device" /> on your{' '}
        <Swap words={SYSTEMS.map((s) => s.word)} i={sys} next={cycle(SYSTEMS.length, setSys)} label="Computer" />.
        <br />
        <span className="fade mt-2 block text-[0.56em] !leading-[1.15] !tracking-[-0.02em]">
          Framed like a real device, ready to{' '}
          <Swap words={ACTIONS} i={act} next={cycle(ACTIONS.length, setAct)} label="Action" />.
        </span>
      </h1>
      <p className="mt-5 flex items-center justify-center gap-2 text-sm text-ink-2">
        Tap a highlighted word to swap it.
      </p>
      <p className="mx-auto mt-6 max-w-[36rem] text-xl leading-snug text-ink-2">
        Handset mirrors and controls your Android phone from a Mac, a Windows PC or Linux, inside a real device frame. Capture
        it, style it, share it.
      </p>
      <div className="mt-8 flex flex-wrap items-center justify-center gap-3">
        <a href={primary.url} className="btn text-lg">
          {primary.label}
        </a>
        <a href={REPO} className="btn btn-light text-lg">
          View on GitHub
        </a>
      </div>
      <p className="label mx-auto mt-7 max-w-[36rem] leading-relaxed text-fade">
        Free and open source · Apache-2.0 · macOS 12+ · Windows 10 or 11 · Linux (experimental)
      </p>
    </section>
  )
}

function primaryDownload(release: Release | null, os: Os): { label: string; url: string } {
  const fall = release?.page ?? RELEASES
  if (os === 'windows') return { label: 'Download for Windows', url: release?.windows?.url ?? fall }
  if (os === 'mac') return { label: 'Download for macOS', url: release?.mac?.url ?? fall }
  return { label: 'Get it for Linux', url: '#download' }
}
