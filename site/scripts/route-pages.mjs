// The site is one page that the router fills in, so a host that serves /terms or /privacy has to hand back
// index.html. Giving each of those URLs its own copy (with its own title, description and address) means link
// previews and search results read the right words, and the page still opens where a host has no fallback rule.
import { existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

const root = join(dirname(fileURLToPath(import.meta.url)), '..')
const dist = join(root, 'dist')
const origin = 'https://handsetapp.com'

// The text face is the biggest file the first screen needs, and the browser would only find it after the style
// sheet: asking for it up front keeps the words from jumping when it arrives.
const font = readdirSync(join(dist, 'assets')).find((f) => /^AlloVariableFriendly.*\.woff2$/.test(f))
const preload = font ? `<link rel="preload" href="/assets/${font}" as="font" type="font/woff2" crossorigin />\n    ` : ''
let base = readFileSync(join(dist, 'index.html'), 'utf8')
base = base.replace('</head>', () => `${preload}</head>`)
writeFileSync(join(dist, 'index.html'), base)

// The AI agents guide is written out as HTML as well (see src/prerender.tsx), so what a crawler that does not run
// scripts reads is the guide itself, and the page then takes over from it.
const ssrEntry = join(root, 'dist-ssr', 'prerender.js')
if (!existsSync(ssrEntry)) throw new Error('dist-ssr/prerender.js is missing: run `vite build --ssr src/prerender.tsx --outDir dist-ssr` first')
const agents = (await import(pathToFileURL(ssrEntry).href)).renderAgentsPage()

const pages = [
  {
    path: 'terms',
    title: 'Terms of Service | Handset',
    description: 'The terms for using the Handset app and website: the Apache-2.0 licence, fair use, and no warranty.',
  },
  {
    path: 'faq',
    title: 'FAQ | Handset',
    description: 'Fixes for the usual problems: Wi-Fi pairing that fails, a phone that does not show up, and a restart of adb.',
  },
  {
    path: 'changelog',
    title: 'Changelog | Handset',
    description: 'What changed in each release of Handset, newest first.',
  },
  {
    path: 'privacy',
    title: 'Privacy Policy | Handset',
    description: 'What the Handset app and website do with your data: nothing is collected, and no account is needed.',
  },
  {
    path: 'agents',
    title: agents.title,
    description: agents.description,
    keywords: agents.keywords,
    ogType: 'article',
    html: agents.html,
    jsonLd: agents.jsonLd,
  },
]

const swap = (html, pattern, value) => {
  if (!pattern.test(html)) throw new Error(`index.html has no ${pattern}`)
  return html.replace(pattern, value)
}
// for what may hold a `$` (a replacement string would read `$1` and `$&` in it as something else)
const put = (html, pattern, value) => {
  if (!pattern.test(html)) throw new Error(`index.html has no ${pattern}`)
  return html.replace(pattern, () => value)
}
const attr = (s) => s.replace(/&/g, '&amp;').replace(/"/g, '&quot;')

for (const p of pages) {
  let html = base
  html = put(html, /<title>.*?<\/title>/, `<title>${p.title.replace(/&/g, '&amp;')}</title>`)
  html = put(html, /(<meta\s+name="description"\s+content=")[^"]*(")/s, `<meta name="description" content="${attr(p.description)}"`)
  html = put(html, /(<meta property="og:title" content=")[^"]*(")/, `<meta property="og:title" content="${attr(p.title)}"`)
  html = put(html, /(<meta\s+property="og:description"\s+content=")[^"]*(")/s, `<meta property="og:description" content="${attr(p.description)}"`)
  html = swap(html, /(<meta property="og:url" content=")[^"]*(")/, `$1${origin}/${p.path}$2`)
  html = put(html, /(<meta name="twitter:title" content=")[^"]*(")/, `<meta name="twitter:title" content="${attr(p.title)}"`)
  html = put(html, /(<meta\s+name="twitter:description"\s+content=")[^"]*(")/s, `<meta name="twitter:description" content="${attr(p.description)}"`)
  html = swap(html, /(<link rel="canonical" href=")[^"]*(")/, `$1${origin}/${p.path}$2`)
  if (p.keywords) html = put(html, /(<meta name="keywords" content=")[^"]*(")/, `<meta name="keywords" content="${attr(p.keywords)}"`)
  if (p.ogType) html = put(html, /(<meta property="og:type" content=")[^"]*(")/, `<meta property="og:type" content="${p.ogType}"`)
  if (p.html) html = put(html, /<div id="root"><\/div>/, `<div id="root">${p.html}</div>`)
  // `<` is written out so nothing in the data can end the script early
  if (p.jsonLd) html = put(html, /<\/head>/, `<script type="application/ld+json">${JSON.stringify(p.jsonLd).replace(/</g, '\\u003c')}</script>\n  </head>`)
  mkdirSync(join(dist, p.path), { recursive: true })
  writeFileSync(join(dist, p.path, 'index.html'), html)
}

// a sitemap and robots file, so search engines find the four pages
const sitemap = ['', ...pages.map((p) => p.path)]
const today = new Date().toISOString().slice(0, 10)
writeFileSync(
  join(dist, 'sitemap.xml'),
  `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n${sitemap
    .map((path) => `  <url><loc>${origin}/${path}</loc><lastmod>${today}</lastmod></url>`)
    .join('\n')}\n</urlset>\n`,
)
writeFileSync(join(dist, 'robots.txt'), `User-agent: *\nAllow: /\n\nSitemap: ${origin}/sitemap.xml\n`)

// hosts that understand this file (Cloudflare Pages, Netlify) send any other path to the app
writeFileSync(join(dist, '_redirects'), '/*  /index.html  200\n')
console.log(`wrote ${pages.map((p) => `/${p.path}`).join(', ')}, sitemap.xml, robots.txt and _redirects`)
