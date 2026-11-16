/** One short, large, two-tone line: what Handset is. The scrcpy details are in the About section. */
export function Story() {
  return (
    <section className="mx-auto max-w-4xl px-5 py-10 text-center sm:px-8 md:py-16">
      <p className="display text-[clamp(1.8rem,4vw,3rem)] !leading-[1.12] !tracking-[-0.02em]">
        Your Android phone, <span className="hl-flat">in a frame that looks like the real thing</span>.{' '}
        <span className="fade">Click, type, capture, share.</span>
      </p>
    </section>
  )
}
