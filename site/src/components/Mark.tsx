import { useEffect, useId, useRef } from 'react'

/** The Handset mark: a phone with two eyes in front of an Android-green and an iOS-blue echo. The eyes follow the pointer. */
export function Mark({ className = '', follow = true }: { className?: string; follow?: boolean }) {
  const svg = useRef<SVGSVGElement>(null)
  const eyes = useRef<SVGGElement>(null)
  const id = useId()

  useEffect(() => {
    if (!follow || window.matchMedia('(prefers-reduced-motion: reduce)').matches) return
    const move = (e: PointerEvent) => {
      const box = svg.current?.getBoundingClientRect()
      if (!box || !eyes.current) return
      const dx = e.clientX - (box.left + box.width / 2)
      const dy = e.clientY - (box.top + box.height / 2)
      const d = Math.hypot(dx, dy) || 1
      const pull = Math.min(1, d / 220)
      eyes.current.style.transform = `translate(${((dx / d) * 8 * pull).toFixed(2)}px, ${((dy / d) * 5 * pull).toFixed(2)}px)`
    }
    window.addEventListener('pointermove', move)
    return () => window.removeEventListener('pointermove', move)
  }, [follow])

  return (
    <svg ref={svg} viewBox="10 13 200 200" className={className} role="img" aria-label="Handset">
      <defs>
        <mask id={id} maskUnits="userSpaceOnUse" x="0" y="0" width="220" height="220">
          <rect width="220" height="220" fill="#fff" />
          <rect x="81" y="53" width="98" height="146" rx="25" fill="#000" />
        </mask>
      </defs>
      <g mask={`url(#${id})`}>
        <g transform="rotate(-12 86 108)">
          <rect x="46" y="44" width="80" height="128" rx="20" fill="#3DDC84" />
          <rect x="53" y="51" width="66" height="114" rx="14" fill="#27B566" />
        </g>
        <g transform="rotate(10 120 98)">
          <rect x="80" y="34" width="80" height="128" rx="20" fill="#0A84FF" />
          <rect x="87" y="41" width="66" height="114" rx="14" fill="#0063D1" />
        </g>
      </g>
      <rect x="88" y="60" width="84" height="132" rx="16" fill="#F4F5F7" />
      <rect x="95" y="67" width="70" height="118" rx="10" fill="#2A2C30" />
      <g ref={eyes} style={{ transition: 'transform 140ms cubic-bezier(0.25, 1, 0.5, 1)' }}>
        <circle cx="120" cy="130" r="5" fill="#fff" />
        <circle cx="140" cy="130" r="5" fill="#fff" />
      </g>
    </svg>
  )
}
