import { useEffect, useState } from 'react'
import { REPO } from '../links'

/** The last call: a dark card with the one thing to do. */
export function Closing() {
  return (
    <section className="mx-auto max-w-6xl px-5 pb-16 sm:px-8 md:pb-24">
      <div className="rounded-[2rem] bg-ink px-6 py-14 text-center text-paper sm:px-12 md:py-20">
        <h2 className="display text-[clamp(2.2rem,6vw,4.2rem)]">
          Plug in <span className="text-marker">your phone.</span>
        </h2>
        <p className="mx-auto mt-5 max-w-lg text-stage-text">
          Handset finds it and the steps update as you go. No cable? Pair over Wi-Fi with a QR code.
        </p>
        <div className="mt-8 flex flex-wrap justify-center gap-3">
          <a href="#download" className="btn btn-green text-lg">
            Download Handset
          </a>
          <a href={REPO} className="btn border-stage-line bg-transparent text-lg text-paper">
            View on GitHub
          </a>
        </div>
      </div>
    </section>
  )
}

/** A small dark pill that follows you down the page, until the download section is on screen. */
export function FloatingDownload() {
  const [show, setShow] = useState(false)
  useEffect(() => {
    let past = false
    let near = false
    const update = () => setShow(past && !near)
    const onScroll = () => {
      past = window.scrollY > 700 && window.scrollY + window.innerHeight < document.body.scrollHeight - 420
      update()
    }
    const target = document.getElementById('download')
    const io = target
      ? new IntersectionObserver(([e]) => {
          near = e.isIntersecting
          update()
        })
      : null
    if (target && io) io.observe(target)
    window.addEventListener('scroll', onScroll, { passive: true })
    onScroll()
    return () => {
      window.removeEventListener('scroll', onScroll)
      io?.disconnect()
    }
  }, [])
  if (!show) return null
  return (
    <a href="#download" className="btn fixed bottom-5 right-5 z-40 !gap-3 !py-3 pl-5 pr-2.5">
      Download
      <span className="grid size-7 place-items-center rounded-full bg-marker text-ink" aria-hidden="true">
        &darr;
      </span>
    </a>
  )
}
