import { useState } from 'react'

const yes = [
  'You demo an Android app and want screenshots and recordings that look finished.',
  'You would rather use a real keyboard and mouse than poke at a small screen.',
  'You show your phone in a call, a talk or a stream.',
  'You want a teammate to watch your phone by link, with nothing to install on their side.',
]
const no = [
  'You need full control of an iPhone or iPad. On macOS they are view only for now.',
  'You want to share your phone over the internet. Share works on your own network for now.',
  'You want a farm of cloud devices. Handset mirrors the phone you are holding.',
]

/** A checklist you tap: what sounds like you. It counts, and says nothing is stored. */
export function ForYou() {
  const [on, setOn] = useState<number[]>([])
  const toggle = (i: number) => setOn((v) => (v.includes(i) ? v.filter((x) => x !== i) : [...v, i]))
  return (
    <section className="mx-auto max-w-5xl px-5 py-12 sm:px-8 md:py-20">
      <h2 className="display h2 text-center">
        Is Handset <span className="hl">for you?</span>
      </h2>
      <p className="mt-4 text-center text-ink-2">Tap what sounds like you.</p>
      <ul className="mx-auto mt-8 grid max-w-3xl gap-3">
        {yes.map((t, i) => {
          const picked = on.includes(i)
          return (
            <li key={t}>
              <button
                type="button"
                aria-pressed={picked}
                onClick={() => toggle(i)}
                className={`flex w-full items-start gap-4 rounded-2xl border-[1.5px] px-5 py-4 text-left transition-colors ${picked ? 'border-ink bg-marker' : 'border-line bg-card hover:border-ink'}`}
              >
                <span
                  className={`mt-1 grid size-6 shrink-0 place-items-center rounded-full border-[1.5px] text-sm ${picked ? 'border-ink bg-ink text-paper' : 'border-fade'}`}
                  aria-hidden="true"
                >
                  {picked ? '✓' : ''}
                </span>
                {t}
              </button>
            </li>
          )
        })}
      </ul>
      <p className="label mt-5 text-center text-ink-2" aria-live="polite">
        {on.length} of {yes.length}
      </p>
      <div className="mx-auto mt-12 max-w-3xl rounded-3xl bg-paper-2 px-6 py-7 sm:px-8">
        <h3 className="display text-xl">Probably not, if</h3>
        <ul className="mt-4 space-y-2.5 text-ink-2">
          {no.map((t) => (
            <li key={t} className="flex gap-3">
              <span className="mt-[0.7em] h-0.5 w-3 shrink-0 bg-ink-2" aria-hidden="true" />
              {t}
            </li>
          ))}
        </ul>
      </div>
    </section>
  )
}
