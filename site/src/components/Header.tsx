import { Link } from '@tanstack/react-router'
import { Mark } from './Mark'
import { RELEASES, REPO } from '../links'
import { useReleaseInfo } from '../release'

const nav = 'label hidden px-3 py-2 hover:bg-marker sm:block'

export function Header() {
  const { release } = useReleaseInfo()
  return (
    <header className="border-b-2 border-ink">
      <div className="mx-auto flex max-w-6xl items-center justify-between px-5 py-3 sm:px-8">
        <Link to="/" className="display flex items-center gap-2.5 text-2xl">
          <Mark className="size-10" />
          Handset
        </Link>
        <nav aria-label="Main" className="flex items-center gap-1 sm:gap-2">
          <Link to="/" hash="features" className={nav}>
            What it does
          </Link>
          <Link to="/" hash="download" className={nav}>
            Download
          </Link>
          <Link to="/changelog" className={nav}>
            Changelog
          </Link>
          <Link to="/faq" className={nav}>
            FAQ
          </Link>
          <a className={`${nav} min-[400px]:block`} href={REPO}>
            GitHub
          </a>
          <a href={release?.page ?? RELEASES} className="btn btn-green ml-2 !px-4 !py-2 text-[0.95rem]">
            {release ? `v${release.version}` : 'Releases'}
          </a>
        </nav>
      </div>
    </header>
  )
}
