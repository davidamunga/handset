import { HeadContent, Outlet, createRootRoute } from '@tanstack/react-router'
import { Footer } from '../components/Footer'
import { Header } from '../components/Header'
import { ReleaseProvider } from '../release'

export const Route = createRootRoute({
  component: Root,
})

function Root() {
  return (
    <ReleaseProvider>
      <HeadContent />
      <a
        href="#main"
        className="sr-only focus:not-sr-only focus:fixed focus:left-4 focus:top-4 focus:z-50 focus:rounded-lg focus:bg-ink focus:px-4 focus:py-2 focus:text-paper"
      >
        Skip to content
      </a>
      <Header />
      <main id="main">
        <Outlet />
      </main>
      <Footer />
    </ReleaseProvider>
  )
}
