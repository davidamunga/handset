import { Link, createFileRoute } from '@tanstack/react-router'
import { LegalPage } from '../components/LegalPage'
import { REPO } from '../links'

export const Route = createFileRoute('/terms')({
  head: () => ({
    meta: [
      { title: 'Terms of Service | Handset' },
      { name: 'description', content: 'The terms for using the Handset app and website: the Apache-2.0 licence, fair use, and no warranty.' },
    ],
  }),
  component: Terms,
})

function Terms() {
  return (
    <LegalPage
      title="Terms of Service"
      updated="October 27, 2026"
      intro="These terms cover this website and the Handset app. Handset is free and open source. By using either one, you agree to them."
    >
      <h2>Who is behind Handset</h2>
      <p>
        Handset is made and maintained by <a href="https://github.com/davidamunga">David Amunga</a>. In these terms, &ldquo;the
        author&rdquo; means him, and &ldquo;Handset&rdquo; means the app, its source code and this website.
      </p>

      <h2>The software and its licence</h2>
      <p>
        The Handset app is released under the <a href={`${REPO}/blob/main/LICENSE`}>Apache License, Version 2.0</a>. That licence, not
        these terms, decides what you may do with the software: you may use, copy, change and share it, under the conditions the
        licence sets out. Where these terms and the licence differ about the software, the licence wins. These terms mainly cover the
        website and how Handset is meant to be used.
      </p>
      <p>
        Handset includes the unmodified server from <a href="https://github.com/Genymobile/scrcpy">scrcpy</a> (Genymobile and
        Romain Vimont, Apache-2.0) and the Inter font (SIL Open Font License 1.1). The{' '}
        <a href={`${REPO}/blob/main/NOTICE`}>notices file</a> lists them. They keep their own licences.
      </p>

      <h2>Using Handset responsibly</h2>
      <p>You agree to use Handset only:</p>
      <ul>
        <li>with devices that you own, or that their owner has allowed you to use;</li>
        <li>in line with the laws that apply to you, including those on recording other people, on copyright and on privacy;</li>
        <li>without trying to get around the protections of another service. Some apps block screen capture, and Handset does not
          remove that block.</li>
      </ul>
      <p>
        Turning on USB debugging or wireless debugging lets a connected computer control your phone. Do that only on computers you
        trust, and turn it off when you do not need it. You are responsible for what you capture, record and share, and for getting
        the consent that the law asks for before recording or sharing other people&apos;s screens, messages or voices.
      </p>

      <h2>Downloads</h2>
      <p>
        Releases are published on <a href={`${REPO}/releases`}>GitHub</a>, with a checksum and a signature for each file. The macOS
        app is signed and notarised. The Windows build is not code-signed yet, so Windows may warn you the first time you open it. Linux has no release file yet: you build it from source.
        Download Handset from the project&apos;s GitHub page, the Homebrew tap or the Scoop bucket named on this site, and check
        the checksum if you want to be sure.
      </p>

      <h2>No warranty</h2>
      <p>
        Handset is provided <strong>&ldquo;as is&rdquo;</strong>, without warranty of any kind, express or implied, including
        that it works with your phone, that it is free of errors, or that it is fit for a particular purpose. It depends on other
        software and on how phones behave, which can change without notice. This is also set out in Section 7 of the Apache-2.0
        licence.
      </p>

      <h2>Limit of liability</h2>
      <p>
        To the fullest extent the law allows, the author is not liable for any loss or damage that comes from using or not being able
        to use Handset or this website, including lost data, lost profit or damage to a device. Nothing in these terms limits a
        right you have by law that cannot be waived.
      </p>

      <h2>Trademarks</h2>
      <p>
        Android is a trademark of Google LLC. iPhone, iPad and macOS are trademarks of Apple Inc. Windows is a trademark of
        Microsoft Corporation. Linux is the registered trademark of Linus Torvalds. scrcpy and Genymobile belong to their owners. These names are used only to say what Handset works
        with. Handset is an independent project and is not affiliated with, endorsed by or sponsored by any of them. The Handset
        name and logo may not be used to suggest that your own product comes from, or is approved by, the author.
      </p>

      <h2>Changes to Handset and to these terms</h2>
      <p>
        Handset may change, or stop being maintained, at any time. These terms may change too. The new version goes on this page with
        a new date, and using Handset after that means you accept it. Earlier versions are in the history of the{' '}
        <a href={`${REPO}/commits/main/site/src/routes/terms.tsx`}>site&apos;s source</a>.
      </p>

      <h2>Contact</h2>
      <p>
        For questions, bugs and requests, open an <a href={`${REPO}/issues`}>issue on GitHub</a>. To report a security problem in
        private, use <a href={`${REPO}/security/advisories/new`}>private vulnerability reporting</a>. How the app handles your data is
        in the <Link to="/privacy">Privacy Policy</Link>.
      </p>
    </LegalPage>
  )
}
