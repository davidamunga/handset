import { Link } from '@tanstack/react-router'
import { Mark } from './Mark'
import { AUTHOR, REPO } from '../links'

const link = 'label hover:text-marker'

export function Footer() {
  return (
    <footer className="bg-stage text-stage-text">
      <div className="mx-auto max-w-6xl px-5 py-12 sm:px-8">
        <div className="flex flex-wrap items-center justify-between gap-6">
          <div className="display flex items-center gap-2.5 text-2xl text-paper">
            <Mark className="size-10" />
            Handset
          </div>
          <nav aria-label="Footer" className="flex flex-wrap gap-x-6 gap-y-3">
            <a className={link} href={REPO}>GitHub</a>
            <a className={link} href={`${REPO}/releases`}>Releases</a>
            <Link className={link} to="/faq">FAQ</Link>
            <Link className={link} to="/changelog">Changelog</Link>
            <a className={link} href={`${REPO}/issues`}>Report a problem</a>
            <a className={link} href={`${REPO}/blob/main/SECURITY.md`}>Security</a>
            <a className={link} href={`${REPO}/blob/main/NOTICE`}>Notices</a>
            <Link className={link} to="/terms">Terms</Link>
            <Link className={link} to="/privacy">Privacy</Link>
          </nav>
        </div>
        <p className="mt-8 max-w-3xl border-t-2 border-stage-line pt-6 text-sm leading-relaxed">
          © 2026 <a className="underline underline-offset-4 hover:text-marker" href={AUTHOR.url}>{AUTHOR.name}</a> (<a className="underline underline-offset-4 hover:text-marker" href={AUTHOR.social}>{AUTHOR.handle}</a>). Handset is Apache-2.0 licensed. scrcpy is © Genymobile and Romain Vimont. Android is a
          trademark of Google LLC. iPhone, iPad and macOS are trademarks of Apple Inc. Windows is a trademark of
          Microsoft. Linux is the registered trademark of Linus Torvalds. They are named here only to say what Handset works with, and none of those companies is affiliated
          with Handset.
        </p>
      </div>
    </footer>
  )
}
