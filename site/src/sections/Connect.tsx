const img = (name: string) => `/shared/${name}`

export function Connect() {
  const shots = [
    { src: 'connect-home.png', title: 'Cable', text: 'Plug in and the steps update as you go. The logo looks around while it waits.' },
    { src: 'connect-wifi.png', title: 'Wi-Fi', text: 'Phones that are paired on your network show up to tap, or type an address.' },
    { src: 'connect-qr.png', title: 'QR code', text: 'Scan a code with the phone to pair over Wi-Fi, no cable at all.' },
  ]
  return (
    <section className="bg-paper-2 py-16 md:py-24">
      <div className="mx-auto max-w-6xl px-5 sm:px-8">
        <div className="max-w-2xl">
          <h2 className="font-display text-[clamp(2rem,4.5vw,3.25rem)] font-bold leading-none tracking-[-0.03em]">
            Connect any way you like
          </h2>
          <p className="mt-4 text-ink-2">
            With no phone connected, Handset opens on a connect screen inside the frame. It also shows an iPhone or iPad
            on macOS, with sound. That one is view only for now.
          </p>
        </div>
        <div className="mt-12 grid gap-10 sm:grid-cols-3 sm:gap-6">
          {shots.map((s) => (
            <figure key={s.src} className="mx-auto max-w-[17rem] sm:max-w-none">
              <img src={img(s.src)} alt={`The ${s.title} connect screen`} width={529} height={1000} loading="lazy" className="mx-auto w-full max-w-[15rem]" />
              <figcaption className="mt-4 text-center">
                <span className="font-display text-lg font-bold">{s.title}</span>
                <span className="mx-auto mt-1 block max-w-[16rem] text-[0.95rem] text-ink-2">{s.text}</span>
              </figcaption>
            </figure>
          ))}
        </div>
      </div>
    </section>
  )
}
