import { Link } from '@tanstack/react-router'
import { Mark } from './Mark'
import { RELEASES, REPO } from '../links'
import { useReleaseInfo } from '../release'

export function Header() {
  const { release } = useReleaseInfo()
  return (
    <header className="mx-auto flex max-w-6xl items-center justify-between px-5 py-5 sm:px-8">
      <Link to="/" className="flex items-center gap-2.5 font-display text-xl font-bold tracking-tight">
        <Mark className="size-9" />
        Handset
      </Link>
      <nav aria-label="Main" className="flex items-center gap-1 text-[0.95rem] sm:gap-2">
        <Link to="/" hash="features" className="hidden rounded-lg px-3 py-2 hover:bg-paper-2 sm:block">
          What it does
        </Link>
        <Link to="/" hash="download" className="hidden rounded-lg px-3 py-2 hover:bg-paper-2 sm:block">
          Download
        </Link>
        <Link to="/changelog" className="hidden rounded-lg px-3 py-2 hover:bg-paper-2 sm:block">
          Changelog
        </Link>
        <a className="hidden rounded-lg px-3 py-2 hover:bg-paper-2 min-[400px]:block" href={REPO}>
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
