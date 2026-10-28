import { createFileRoute } from '@tanstack/react-router'
import { LegalPage } from '../components/LegalPage'
import { REPO } from '../links'

export const Route = createFileRoute('/faq')({
  head: () => ({
    meta: [
      { title: 'FAQ | Handset' },
      { name: 'description', content: 'Fixes for the usual problems: Wi-Fi pairing that fails, a phone that does not show up, a restart of adb, and Linux.' },
    ],
  }),
  component: Faq,
})

function Faq() {
  return (
    <LegalPage
      title="FAQ"
      updated="October 27, 2026"
      intro="Most problems come from the connection to the phone, not from Handset. Here are the fixes that work most often."
      stamp={false}
    >
      <h2>Wi-Fi pairing fails or never finishes</h2>
      <p>Work through these in order. Each one fixes a common cause.</p>
      <ul>
        <li>
          <strong>Restart adb.</strong> adb is the tool that talks to your phone, and it can get stuck. Stopping it and starting it
          again fixes most pairing problems. Handset now does this once by itself when pairing fails. To do it by hand, run{' '}
          <code>adb kill-server</code>, then <code>adb start-server</code>.
        </li>
        <li>
          <strong>Use the new code.</strong> The code on the pairing screen changes each time you open it, and it stops working when
          you close the screen. Open <strong>Developer options</strong>, then <strong>Wireless debugging</strong>, then{' '}
          <strong>Pair device with pairing code</strong>, and type the code that shows right then.
        </li>
        <li>
          <strong>Use the pairing port, not the connection port.</strong> The pairing screen shows its own address and port. It is
          not the same as the port on the main Wireless debugging screen.
        </li>
        <li>
          <strong>Same network.</strong> The phone and the computer must be on the same Wi-Fi. A guest network, a VPN, or a router
          setting that isolates devices will block the connection.
        </li>
        <li>
          <strong>Turn Wireless debugging off and on.</strong> Then open the pairing screen again and try with the new code.
        </li>
        <li>
          <strong>Try the QR code.</strong> On the Handset start screen, choose the QR code and scan it from{' '}
          <strong>Pair device with QR code</strong> on the phone. You type nothing, so there is no code to get wrong.
        </li>
      </ul>

      <h2>Does Handset run on Linux?</h2>
      <p>
        Yes, but it is experimental. It builds and passes its tests on Ubuntu 22.04, Alpine and with Nix, and the same code runs on
        Debian 12 and newer, Fedora and Arch. It has not been run on a real Linux desktop yet, so the transparent phone-shaped window
        may misbehave on some desktops. Releases after 0.8.1 include a tarball for x86_64 and aarch64 (glibc 2.35 or newer, with
        ALSA); on other systems you build it from source. The <a href={`${REPO}#linux-experimental`}>Linux guide</a> explains both.
      </p>
      <ul>
        <li>
          <strong>Phone not found over USB.</strong> Linux needs a rule that lets your user use the phone. Install{' '}
          <code>android-udev-rules</code>, or your distribution&apos;s <code>android-tools</code> package, then unplug the phone and
          plug it in again.
        </li>
        <li>
          <strong>Copy does nothing.</strong> Handset uses small helper programs: a clipboard tool (<code>wl-clipboard</code> on
          Wayland, <code>xclip</code> on X11) and <code>zenity</code> or <code>kdialog</code> for error dialogs.{' '}
          <code>handset --doctor</code> lists any that are missing.
        </li>
        <li>
          <strong>The phone is tiled or squashed.</strong> On a tiling desktop such as Hyprland, make the window float. In Hyprland
          the class is <code>handset</code>. The rule is <code>windowrulev2 = float, class:^(handset)$</code> on older versions and{' '}
          <code>windowrule = float on, match:class ^(handset)$</code> on newer ones; check <code>hyprctl version</code>.
        </li>
      </ul>

      <h2>The error says &ldquo;protocol fault&rdquo;</h2>
      <p>
        The phone closed the connection during pairing. This nearly always means the code was wrong or has expired, or the pairing
        screen was closed and opened again. Get a new code and try again. If a new code does not help, restart adb as above.
      </p>

      <h2>Pairing works but the phone does not show up</h2>
      <p>
        Pairing only introduces the phone to the computer once. You then connect to it from the Wi-Fi list on the start screen, or
        with the address shown on the main Wireless debugging screen. If the phone is paired but missing, check that Wireless
        debugging is still on. Android turns it off when you change Wi-Fi network.
      </p>

      <h2>My phone does not show up over USB</h2>
      <ul>
        <li>Use a cable that carries data. Some cables only charge.</li>
        <li>Turn on <strong>USB debugging</strong> in Developer options, and allow the computer when the phone asks.</li>
        <li>If the phone says it is <code>unauthorized</code>, unlock it and accept the prompt. Choose &ldquo;Revoke USB debugging authorizations&rdquo; first if no prompt appears.</li>
        <li>Restart adb as above, then unplug and plug in the cable again.</li>
      </ul>

      <h2>Will restarting adb disturb anything?</h2>
      <p>
        It drops every adb connection for a few seconds, including other tools that use adb, such as Android Studio. Phones that
        are paired or on USB connect again by themselves. Handset only restarts adb after a pairing attempt has failed, and only once.
      </p>

      <h2>Still stuck?</h2>
      <p>
        Open an <a href={`${REPO}/issues`}>issue on GitHub</a> and say which phone and Android version you have, what you tried, and
        the exact message Handset showed.
      </p>
    </LegalPage>
  )
}
