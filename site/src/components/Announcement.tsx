import { useState } from 'react'
import { Link } from '@tanstack/react-router'
import { useReleaseInfo } from '../release'

/** A dark strip above the page: what the newest release is, and where to read about it. */
export function Announcement() {
  const { release } = useReleaseInfo()
  const [shown, setShown] = useState(true)
  if (!release || !shown) return null
  return (
    <div className="bg-ink text-paper">
      <div className="mx-auto flex max-w-6xl 2xl:max-w-7xl items-center justify-center gap-3 px-5 py-2 text-[0.92rem] sm:px-8">
        <span className="size-2 shrink-0 rounded-full bg-android" aria-hidden="true" />
        <p className="min-w-0 truncate">
          Handset <strong className="font-semibold">v{release.version}</strong> is out
        </p>
        <Link to="/changelog" className="shrink-0 underline underline-offset-4 hover:text-marker">
          What&apos;s new
        </Link>
        <button
          type="button"
          onClick={() => setShown(false)}
          aria-label="Hide this notice"
          className="ml-1 shrink-0 rounded-full px-2 text-lg leading-none text-stage-text hover:text-paper"
        >
          &times;
        </button>
      </div>
    </div>
  )
}
