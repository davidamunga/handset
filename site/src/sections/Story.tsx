/** A large, two-tone paragraph: what Handset is, in the voice of the page. */
export function Story() {
  return (
    <section className="mx-auto max-w-4xl px-5 py-12 sm:px-8 md:py-20">
      <p className="display text-[clamp(1.7rem,3.6vw,2.6rem)] !leading-[1.15] !tracking-[-0.02em]">
        Handset puts your Android phone on your desktop, <span className="hl-flat">in a frame that looks like the real thing</span>.{' '}
        <span className="fade">
          It speaks the scrcpy protocol and embeds the unmodified scrcpy server, so you click, scroll and type on your keyboard
          while the phone does the work. The window is transparent, so the phone sits on your desktop like an object, not a
          rectangle. Capture it, style it, or send a link so someone else can watch.
        </span>
      </p>
    </section>
  )
}
