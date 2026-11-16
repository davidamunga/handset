import { createFileRoute } from '@tanstack/react-router'
import { AgentsGuide } from '../components/AgentsGuide'
import { AGENTS_DESCRIPTION, AGENTS_TITLE } from '../content/agents'

// The build also writes this page as plain HTML (scripts/route-pages.mjs), with the guide's words in it and its
// structured data, so search engines and AI crawlers read it without running the page.
export const Route = createFileRoute('/agents')({
  head: () => ({
    meta: [{ title: AGENTS_TITLE }, { name: 'description', content: AGENTS_DESCRIPTION }],
  }),
  component: AgentsGuide,
})
