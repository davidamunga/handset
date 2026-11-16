import { Mark } from './Mark'

type Look = { body: string; screen: string; cut: 'dot' | 'notch' | 'pill' | 'none'; rot: number; wide?: boolean; lift?: number }

// a few of the built-in frames, drawn flat: the real ones are in the app
const row: Look[] = [
  { body: '#2b2d31', screen: '#a9c4e6', cut: 'dot', rot: -9, lift: 10 },
  { body: '#eceae4', screen: '#f2b9a0', cut: 'notch', rot: 6 },
  { body: '#8f9c8a', screen: '#3e4a52', cut: 'dot', rot: -5, lift: 14 },
  { body: '#e9a894', screen: '#2b2d31', cut: 'pill', rot: 8, wide: true },
  { body: '#b9a7d6', screen: '#f4e3a1', cut: 'dot', rot: -7, lift: 12 },
  { body: '#17181b', screen: '#7fd1ae', cut: 'pill', rot: 7 },
]

function Phone({ look, hideSmall = false }: { look: Look; hideSmall?: boolean }) {
  return (
    <div
      className={`float shrink-0 rounded-[1.15rem] p-[3px] ${hideSmall ? 'hidden sm:block' : ''} ${look.wide ? 'w-[4.6rem] sm:w-[5.6rem]' : 'w-[2.9rem] sm:w-[3.6rem]'} `}
      style={{
        aspectRatio: look.wide ? '1.02' : '0.5',
        background: look.body,
        transform: `rotate(${look.rot}deg) translateY(${look.lift ?? 0}px)`,
      }}
    >
      <div className="relative size-full rounded-[0.95rem]" style={{ background: look.screen }}>
        {look.cut === 'dot' && <span className="absolute left-1/2 top-[5%] size-[7%] -translate-x-1/2 rounded-full" style={{ background: look.body }} />}
        {look.cut === 'pill' && <span className="absolute left-1/2 top-[4%] h-[5%] w-[28%] -translate-x-1/2 rounded-full" style={{ background: look.body }} />}
        {look.cut === 'notch' && <span className="absolute left-1/2 top-0 h-[6%] w-[34%] -translate-x-1/2 rounded-b-lg" style={{ background: look.body }} />}
      </div>
    </div>
  )
}

/** A row of little phones with the Handset mark among them, marked "yours" in a hand-drawn way. */
export function FrameRow() {
  const left = row.slice(0, 3)
  const right = row.slice(3)
  return (
    <div className="relative mx-auto flex max-w-xl items-end justify-center gap-2 pt-8 sm:gap-3" aria-hidden="true">
      {left.map((l, i) => (
        <Phone key={i} look={l} hideSmall={i === 0} />
      ))}
      <div className="relative mx-1 sm:mx-3">
        <span className="label absolute -top-7 left-[-1.2rem] -rotate-6 text-ink-2 normal-case">yours</span>
        <svg viewBox="0 0 60 40" className="absolute -top-8 left-5 h-8 w-12 text-ink-2" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round">
          <path d="M4 6 C 22 0, 40 6, 46 28" />
          <path d="M38 24 L46 29 L50 20" />
        </svg>
        <div className="rounded-[1.6rem] border-2 border-dashed border-ink bg-marker/50 p-2 sm:p-3">
          <Mark className="size-16 sm:size-20" />
        </div>
      </div>
      {right.map((l, i) => (
        <Phone key={i} look={l} hideSmall={i === right.length - 1} />
      ))}
    </div>
  )
}
