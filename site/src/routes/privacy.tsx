import { createFileRoute } from '@tanstack/react-router'
import { LegalPage } from '../components/LegalPage'
import { REPO } from '../links'

export const Route = createFileRoute('/privacy')({
  head: () => ({
    meta: [
      { title: 'Privacy Policy | Handset' },
      { name: 'description', content: 'What the Handset app and website do with your data: nothing is collected, and no account is needed.' },
    ],
  }),
  component: Privacy,
})

function Privacy() {
  return (
    <LegalPage
      title="Privacy Policy"
      updated="November 2, 2026"
      intro="Handset does not collect your data. This page says what the app and this website do, so you do not have to take that on trust."
    >
      <h2>The short version</h2>
      <ul>
        <li>Handset has no account, no sign-in, no analytics and no advertising.</li>
        <li>What you mirror, record or screenshot stays on your computer.</li>
        <li>The only time the app talks to the internet is to ask GitHub whether a newer version exists, and to download an update if you choose to install one.</li>
        <li>Share, which shows your phone to someone on your network, is off until you start it, and each person is allowed by you before they see anything.</li>
        <li>This website sets no cookies and uses no trackers.</li>
      </ul>

      <h2>The Handset app</h2>
      <p>
        Handset runs on your Mac, Windows PC or Linux computer. It talks to your phone through <strong>adb</strong>, over a USB cable or your local
        network, and shows the phone&apos;s screen and sound in a window. Nothing on the screen, no sound, no clipboard text and no
        keystroke is sent to the author or to anyone else. It all stays between your phone and your computer.
      </p>
      <p>
        When you mirror an Android phone, Handset puts a small helper program (the scrcpy server) on the phone for the length of the
        session. When you mirror an iPhone or iPad on a Mac, the screen is read through the system&apos;s own screen capture for devices.
      </p>
      <p>
        <strong>Files it writes.</strong> Screenshots and recordings are saved in your Pictures and Movies (or Videos) folder, in a
        folder called handset. Your settings, saved presets and a key used to check updates are kept in{' '}
        <code>~/.config/handset</code>. You can delete any of these at any time. If you turn on the option to copy a screenshot to the
        clipboard, that goes to your own clipboard.
      </p>
      <p>
        <strong>Clipboard sharing.</strong> Handset can pass copied text between your phone and your computer. It moves directly
        between the two and goes nowhere else.
      </p>

      <h2>Share in a browser</h2>
      <p>
        <strong>Share</strong> is off until you choose it from the menu. Then Handset opens a small web server on your computer, on the
        network it is connected to, and the link it gives you points there. Someone who opens the link connects straight to your
        computer. Nothing goes through a server of ours, and Handset contacts no third-party service to make the connection.
      </p>
      <p>
        Before anyone sees anything, Handset asks you about that person, showing their network address. You can let them watch,
        let them watch and control the phone, or refuse. Someone you allow sees everything on the phone&apos;s screen, including
        notifications and anything typed, and hears what it plays. With control they can also tap, type and press the phone&apos;s
        keys. Sharing stops when you choose <strong>Stop Sharing</strong>, close the window or disconnect the phone.
      </p>
      <p>
        The page and the setup of the connection travel over plain HTTP on your network. The picture and the sound are encrypted.
        Because of that, share only on a network you trust. A viewer&apos;s page keeps its own style choices in that
        person&apos;s browser, on their device, and sends them nowhere. The page links to this project on GitHub; nothing is
        loaded from there until the person follows the link. More in the{' '}
        <a href={`${REPO}/blob/main/SECURITY.md`}>security notes</a> and the{' '}
        <a href={`${REPO}/blob/main/docs/share.md`}>Share guide</a>.
      </p>

      <h2>Checking for updates</h2>
      <p>
        On every start, Handset asks GitHub (<code>api.github.com</code>) for the latest
        release. The request carries a user agent that names the app and its version, for example <code>handset/0.7.0</code>, and
        nothing else about you. As with any request, GitHub sees your IP address, and handles it as described in{' '}
        <a href="https://docs.github.com/en/site-policy/privacy-policies/github-general-privacy-statement">GitHub&apos;s privacy statement</a>.
      </p>
      <p>
        You can turn this off in the menu (<strong>Check for Updates on Start</strong>), with the <code>--no-update-check</code> option,
        or with <code>update-check=off</code> in the settings file. If you choose to install an update, Handset downloads it from the
        project&apos;s GitHub releases and checks its signature and checksum before it replaces anything.
      </p>

      <h2>Other software Handset uses</h2>
      <p>
        Handset runs <strong>adb</strong> (Android platform-tools) and, for recordings, <strong>ffmpeg</strong>, which are installed
        on your computer. They run locally. Each has its own terms, and Handset passes nothing to them except what is needed to talk to
        your phone and to write your files.
      </p>

      <h2>This website</h2>
      <p>
        The site is a static page. It sets <strong>no cookies</strong>, runs <strong>no analytics</strong>, and has no forms or
        accounts. Its fonts are served from the site itself, so no font service sees you visit.
      </p>
      <p>
        To show the newest version and link to the right download, your browser asks GitHub for the latest release when the page
        loads. GitHub sees the request as described above. If you click a download or a GitHub link, you are then on GitHub and under
        its policies. The site is delivered by a static hosting provider, which, like any web server, may keep ordinary access logs
        (such as IP address, time and the page asked for) for a short time to keep the service running and safe.
      </p>

      <h2>Children</h2>
      <p>Handset and this site are not aimed at children, and they collect nothing from anyone.</p>

      <h2>Changes</h2>
      <p>
        If this policy changes, the new version goes on this page with a new date. Earlier versions are in the history of the{' '}
        <a href={`${REPO}/commits/main/site/src/routes/privacy.tsx`}>site&apos;s source</a>.
      </p>

      <h2>Contact</h2>
      <p>
        Questions about privacy can go in an <a href={`${REPO}/issues`}>issue on GitHub</a>. To report a security problem privately,
        use <a href={`${REPO}/security/advisories/new`}>private vulnerability reporting</a>.
      </p>
    </LegalPage>
  )
}
