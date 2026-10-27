// The site shows the same pictures as the README and ships the same icon, so they are copied from where the
// repository keeps them (into the git-ignored public/shared) instead of being committed a second time.
import { cpSync, mkdirSync, rmSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = join(dirname(fileURLToPath(import.meta.url)), '..', '..')
const out = join(root, 'site', 'public', 'shared')
rmSync(out, { recursive: true, force: true })
mkdirSync(out, { recursive: true })

const files = {
  'docs/images/connect-home.png': 'connect-home.png',
  'docs/images/connect-wifi.png': 'connect-wifi.png',
  'docs/images/connect-qr.png': 'connect-qr.png',
  'docs/images/editor.png': 'editor.png',
  'docs/images/styled-example.jpg': 'styled-example.jpg',
  'docs/images/mirroring.png': 'mirroring.png',
  'docs/images/handset-looking.gif': 'handset-looking.gif',
  'docs/images/og/og-product.png': 'og.png',
  'app/packaging/icon/handset-icon.svg': 'icon.svg',
  'app/packaging/icon/handset-icon-512.png': 'icon-512.png',
  'app/packaging/icon/handset-mark.svg': 'mark.svg',
}
for (const [from, to] of Object.entries(files)) cpSync(join(root, from), join(out, to))
console.log(`synced ${Object.keys(files).length} shared files into site/public/shared`)
