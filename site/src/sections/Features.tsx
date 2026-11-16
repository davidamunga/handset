import { Link } from '@tanstack/react-router'
import { CopyBlock } from '../components/CopyBlock'
import { REPO } from '../links'

const img = (name: string) => `/shared/${name}`

export function Features() {
  return (
    <section id="features" className="mx-auto max-w-6xl 2xl:max-w-7xl scroll-mt-4 px-5 py-12 sm:px-8 md:py-20">
      <h2 className="display h2">
        What you can do <span className="fade">with it</span>
      </h2>

      <Row
        number="01"
        title="Use your phone from the desktop"
        body="Live video and device audio over adb. Click to touch, scroll, type on your keyboard, rotate to landscape, and copy and paste between the two. Plug in a cable, or use Wi-Fi."
        points={['Touch, scroll, keyboard and text input', 'Clipboard shared both ways', 'Starts when you plug the phone in']}
        stage
      >
        <img
          src={img('mirroring.png')}
          alt="Handset mirroring an Android phone inside a device frame, with its toolbar beside it"
          width={880}
          height={1550}
          loading="lazy"
          className="mx-auto h-[min(34rem,70vh)] w-auto"
        />
      </Row>

      <Row
        number="02"
        title="Inside a real device frame"
        body="Twenty-three frames, from punch-hole and notch to foldable, flip and tablet, or bring your own PNG. The window is transparent, so the phone sits on your desktop like an object, not a rectangle."
        points={['Screenshots as transparent PNGs', 'Recordings as MP4 with sound, or GIF', 'Switch frames while it runs']}
        flip
      >
        <figure>
          <img
            src={img('styled-example.jpg')}
            alt="A framed Android phone in front of an orange and blue macOS wallpaper, 16 by 9"
            width={1600}
            height={900}
            loading="lazy"
            className="card w-full"
          />
          <figcaption className="mt-3 text-sm text-ink-2">
            Wallpaper: macOS Ventura, © Apple Inc., shown only as a desktop behind the phone. The phone is a stock
            Android emulator.
          </figcaption>
        </figure>
      </Row>

      <Row
        number="03"
        title="Style every capture"
        body="Put a screenshot or a recording on a gradient, a wallpaper, a colour or your own picture. Set the padding, shadow and corners, pick an aspect ratio from 1:1 to 16:9, and trim a recording while you watch it. Save the look as a preset. Your original is never touched."
        points={['Presets with thumbnails of your own picture', 'Light and dark, following your system', 'Export a PNG, MP4 or GIF']}
      >
        <img
          src={img('editor.png')}
          alt="The Handset style editor: presets, background, layout and aspect ratio controls beside a live preview"
          width={1280}
          height={933}
          loading="lazy"
          className="card w-full"
        />
      </Row>

      <Row
        number="04"
        title="Show it to someone with a link"
        body="Right-click the phone and choose Share. Anyone on your network opens the link in a browser and sees your phone in its frame, with sound. You are asked about each person before they see anything, and you choose whether they may only watch or may tap and type too."
        points={['No app to install on their side', 'Watch only, or allow control, per person', 'They can restyle the page and save a PNG or JPEG']}
        flip
        link={{ href: `${REPO}/blob/main/docs/share.md`, label: 'Read the Share guide' }}
      >
        <figure>
          <img
            src={img('share-page.png')}
            alt="The page Share shows in a browser: the phone in its frame on a dotted background, with the phone's keys beside it and the tools along the top"
            width={1440}
            height={900}
            loading="lazy"
            className="card w-full"
          />
          <figcaption className="mt-3 text-sm text-ink-2">
            The page a viewer sees. The screen is a drawn demo, not a real phone. On the same Wi-Fi or network only, for now.
          </figcaption>
        </figure>
      </Row>

      <Row
        number="05"
        title="Let an AI agent use your phone"
        body="Handset can be an MCP server. Claude Code, Cursor, VS Code, Codex, Gemini CLI and other AI agents can then read your phone's screen, tap, type and open apps, while you watch it work in the frame. It is off until you switch it on, and it stays on your computer."
        points={['Works with the agents you already use', 'Reads the screen as text, so taps are exact', 'You watch it in the window, and can stop it any time']}
        link={{ to: '/agents', label: 'Read the setup guide' }}
      >
        <div className="card p-6 sm:p-8">
          <p className="label text-ink-2">Add it to Claude Code</p>
          <CopyBlock text="claude mcp add handset -- handset mcp" />
          <p className="label mt-6 text-ink-2">Then ask</p>
          <p className="display mt-2 text-[clamp(1.15rem,2vw,1.4rem)] !leading-snug !tracking-[-0.01em]">
            &ldquo;Walk through sign-up on my phone and tell me where a first-time user gets stuck.&rdquo;
          </p>
          <p className="mt-5 text-[0.95rem] text-ink-2">
            The same one line, with small changes, adds it to Cursor, VS Code, Codex, Gemini CLI, Zed and more.
          </p>
        </div>
      </Row>
    </section>
  )
}

function Row(props: {
  number: string
  title: string
  body: string
  points: string[]
  children: React.ReactNode
  stage?: boolean
  flip?: boolean
  link?: { href?: string; to?: string; label: string }
}) {
  return (
    <article className="mt-16 grid items-center gap-8 md:mt-28 md:grid-cols-12 md:gap-14">
      <div className={`md:col-span-5 ${props.flip ? 'md:order-2' : ''}`}>
        <p className="label inline-block rounded-full bg-marker px-3 py-1">{props.number}</p>
        <h3 className="display mt-4 text-[clamp(1.6rem,3vw,2.25rem)] !leading-[1.02]">{props.title}</h3>
        <p className="mt-4 text-ink-2">{props.body}</p>
        <ul className="mt-5 space-y-2.5">
          {props.points.map((p) => (
            <li key={p} className="flex gap-3">
              <span className="mt-[0.62em] size-2.5 shrink-0 rounded-full bg-ink" aria-hidden="true" />
              {p}
            </li>
          ))}
        </ul>
        {props.link && (
          <p className="mt-6">
            {props.link.to ? (
              <Link to={props.link.to} className="btn btn-light !px-4 !py-2 text-[0.95rem]">
                {props.link.label} &rarr;
              </Link>
            ) : (
              <a href={props.link.href} className="btn btn-light !px-4 !py-2 text-[0.95rem]">
                {props.link.label} &rarr;
              </a>
            )}
          </p>
        )}
      </div>
      <div className={`md:col-span-7 ${props.flip ? 'md:order-1' : ''}`}>
        {props.stage ? <div className="rounded-[2rem] bg-ink px-6 py-10 sm:py-12">{props.children}</div> : props.children}
      </div>
    </article>
  )
}
