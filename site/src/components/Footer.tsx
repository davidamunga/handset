import { Link } from '@tanstack/react-router'
import { Mark } from './Mark'
import { AUTHOR, REPO } from '../links'

export function Footer() {
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
            <Link className="hover:text-paper" to="/faq">FAQ</Link>
            <Link className="hover:text-paper" to="/changelog">Changelog</Link>
            <a className="hover:text-paper" href={`${REPO}/issues`}>Report a problem</a>
            <a className="hover:text-paper" href={`${REPO}/blob/main/SECURITY.md`}>Security</a>
            <a className="hover:text-paper" href={`${REPO}/blob/main/NOTICE`}>Notices</a>
            <Link className="hover:text-paper" to="/terms">Terms</Link>
            <Link className="hover:text-paper" to="/privacy">Privacy</Link>
          </nav>
        </div>
        <p className="mt-8 max-w-3xl text-sm leading-relaxed">
          © 2026 <a className="underline underline-offset-4 hover:text-paper" href={AUTHOR.url}>{AUTHOR.name}</a> (<a className="underline underline-offset-4 hover:text-paper" href={AUTHOR.social}>{AUTHOR.handle}</a>). Handset is Apache-2.0 licensed. scrcpy is © Genymobile and Romain Vimont. Android is a
          trademark of Google LLC. iPhone, iPad and macOS are trademarks of Apple Inc. Windows is a trademark of
          Microsoft. Linux is the registered trademark of Linus Torvalds. They are named here only to say what Handset works with, and none of those companies is affiliated
          with Handset.
        </p>
      </div>
    </footer>
  )
}
