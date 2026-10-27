// The site is one page that the router fills in, so a host that serves /terms or /privacy has to hand back
// index.html. Giving each of those URLs its own copy (with its own title, description and address) means link
// previews and search results read the right words, and the page still opens where a host has no fallback rule.
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const dist = join(dirname(fileURLToPath(import.meta.url)), '..', 'dist')
const base = readFileSync(join(dist, 'index.html'), 'utf8')
const origin = 'https://handsetapp.com'

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
]

const swap = (html, pattern, value) => {
  if (!pattern.test(html)) throw new Error(`index.html has no ${pattern}`)
  return html.replace(pattern, value)
}

for (const p of pages) {
  let html = base
  html = swap(html, /<title>.*?<\/title>/, `<title>${p.title}</title>`)
  html = swap(html, /(<meta\s+name="description"\s+content=")[^"]*(")/s, `$1${p.description}$2`)
  html = swap(html, /(<meta property="og:title" content=")[^"]*(")/, `$1${p.title}$2`)
  html = swap(html, /(<meta\s+property="og:description"\s+content=")[^"]*(")/s, `$1${p.description}$2`)
  html = swap(html, /(<meta property="og:url" content=")[^"]*(")/, `$1${origin}/${p.path}$2`)
  html = swap(html, /(<meta name="twitter:title" content=")[^"]*(")/, `$1${p.title}$2`)
  html = swap(html, /(<meta\s+name="twitter:description"\s+content=")[^"]*(")/s, `$1${p.description}$2`)
  html = swap(html, /(<link rel="canonical" href=")[^"]*(")/, `$1${origin}/${p.path}$2`)
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
