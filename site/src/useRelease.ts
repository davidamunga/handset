import { useEffect, useState } from 'react'
import { REPO, RELEASES } from './links'

export type Download = { url: string; size: string }
export type Release = { version: string; page: string; mac?: Download; windows?: Download }

const mb = (bytes: number) => `${(bytes / 1_048_576).toFixed(0)} MB`

/** The newest release from GitHub, so the buttons point at the real files. Null until it arrives, or if it cannot. */
export function useRelease(): Release | null {
  const [release, setRelease] = useState<Release | null>(null)
  useEffect(() => {
    const ctl = new AbortController()
    fetch(`https://api.github.com/repos/${REPO.split('github.com/')[1]}/releases/latest`, { signal: ctl.signal })
      .then((r) => (r.ok ? r.json() : Promise.reject(r.status)))
      .then((j: { tag_name: string; html_url: string; assets: { name: string; size: number; browser_download_url: string }[] }) => {
        const pick = (suffix: string): Download | undefined => {
          const a = j.assets.find((x) => x.name.endsWith(suffix))
          return a && { url: a.browser_download_url, size: mb(a.size) }
        }
        setRelease({
          version: j.tag_name.replace(/^v/, ''),
          page: j.html_url || RELEASES,
          mac: pick('-macos.zip'),
          windows: pick('-windows.zip'),
        })
      })
      .catch(() => {})
    return () => ctl.abort()
  }, [])
  return release
}

export type Platform = 'mac' | 'windows' | 'other'

export function detectPlatform(): Platform {
  if (typeof navigator === 'undefined') return 'other'
  const s = `${navigator.userAgent} ${navigator.platform ?? ''}`
  if (/Mac/i.test(s) && !/iPhone|iPad/i.test(s)) return 'mac'
  if (/Win/i.test(s)) return 'windows'
  return 'other'
}
