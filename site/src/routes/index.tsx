import { createFileRoute } from '@tanstack/react-router'
import { About } from '../sections/About'
import { Closing, FloatingDownload } from '../sections/Closing'
import { Connect } from '../sections/Connect'
import { DownloadSection } from '../sections/DownloadSection'
import { Features } from '../sections/Features'
import { ForYou } from '../sections/ForYou'
import { Hero } from '../sections/Hero'
import { HomeFaq } from '../sections/HomeFaq'
import { Story } from '../sections/Story'

export const Route = createFileRoute('/')({
  head: () => ({ meta: [{ title: 'Handset: your phone, on your desktop' }] }),
  component: Home,
})

function Home() {
  return (
    <>
      <Hero />
      <Story />
      <Features />
      <Connect />
      <ForYou />
      <DownloadSection />
      <HomeFaq />
      <About />
      <Closing />
      <FloatingDownload />
    </>
  )
}
