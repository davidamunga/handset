const img = (name: string) => `/shared/${name}`

export function Features() {
  return (
    <section id="features" className="mx-auto max-w-6xl scroll-mt-4 px-5 py-8 sm:px-8 md:py-16">
      <h2 className="font-display text-[clamp(2rem,4.5vw,3.25rem)] font-bold leading-none tracking-[-0.03em]">
        What you can do with it
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
        body="Twenty-one frames, from punch-hole and notch to foldable, flip and tablet, or bring your own PNG. The window is transparent, so the phone sits on your desktop like an object, not a rectangle."
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
            className="w-full rounded-3xl border border-line"
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
          className="w-full rounded-2xl border border-line"
        />
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
}) {
  return (
    <article className="mt-16 grid items-center gap-8 md:mt-24 md:grid-cols-12 md:gap-12">
      <div className={`md:col-span-5 ${props.flip ? 'md:order-2' : ''}`}>
        <p className="font-display text-sm font-bold tracking-widest text-ink-2">{props.number}</p>
        <h3 className="mt-3 font-display text-[clamp(1.6rem,3vw,2.2rem)] font-bold leading-[1.05] tracking-[-0.02em]">
          {props.title}
        </h3>
        <p className="mt-4 text-ink-2">{props.body}</p>
        <ul className="mt-5 space-y-2">
          {props.points.map((p) => (
            <li key={p} className="flex gap-3">
              <span className="mt-[0.7em] size-1.5 shrink-0 rounded-full bg-ink" aria-hidden="true" />
              {p}
            </li>
          ))}
        </ul>
      </div>
      <div className={`md:col-span-7 ${props.flip ? 'md:order-1' : ''}`}>
        {props.stage ? <div className="rounded-[2rem] bg-stage px-6 py-10 sm:py-12">{props.children}</div> : props.children}
      </div>
    </article>
  )
}
