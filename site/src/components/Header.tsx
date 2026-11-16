import { Link } from '@tanstack/react-router'
import { Mark } from './Mark'
import { RELEASES, REPO } from '../links'
import { useReleaseInfo } from '../release'

const nav = 'hidden whitespace-nowrap rounded-full px-3.5 py-2 text-[0.95rem] text-ink-2 hover:bg-paper-2 hover:text-ink md:block'

export function Header() {
  const { release } = useReleaseInfo()
  return (
    <header className="mx-auto flex max-w-6xl 2xl:max-w-7xl items-center justify-between px-5 py-4 sm:px-8">
      <Link to="/" className="display flex items-center gap-2.5 text-2xl">
        <Mark className="size-10" />
        Handset
      </Link>
      <nav aria-label="Main" className="flex items-center gap-1">
        <Link to="/" hash="features" className={nav}>
          What it does
        </Link>
        <Link to="/" hash="download" className={nav}>
          Download
        </Link>
        <Link to="/agents" className={nav}>
          AI agents
        </Link>
        <Link to="/changelog" className={nav}>
          Changelog
        </Link>
        <Link to="/faq" className={nav}>
          FAQ
        </Link>
        <a className={nav} href={REPO}>
          GitHub
        </a>
        <a href={release?.page ?? RELEASES} className="btn ml-2 shrink-0 whitespace-nowrap !px-4 !py-2 text-[0.95rem]">
          {release ? `v${release.version}` : 'Releases'}
        </a>
      </nav>
    </header>
  )
}
