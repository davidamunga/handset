import { createFileRoute } from '@tanstack/react-router'
import { About } from '../sections/About'
import { Connect } from '../sections/Connect'
import { DownloadSection } from '../sections/DownloadSection'
import { Features } from '../sections/Features'
import { Hero } from '../sections/Hero'

export const Route = createFileRoute('/')({
  head: () => ({ meta: [{ title: 'Handset: your phone, on your desktop' }] }),
  component: Home,
})

function Home() {
  return (
    <>
      <Hero />
      <Features />
      <Connect />
      <DownloadSection />
      <About />
    </>
  )
}
