import { useEffect, useState } from 'react'
import { REPO, RELEASES } from './links'

export type Download = { url: string; size: string }
export type LinuxArch = 'x86_64' | 'aarch64'
/** Downloads of the installable files, across every release. Checksums, signatures and the update archive are left out. */
export type Downloads = { total: number; mac: number; windows: number; linux: number }
export type Release = {
  version: string
  page: string
  downloads: Downloads
  mac?: Download
  windows?: Download
  linux: Partial<Record<LinuxArch, Download>>
}

const mb = (bytes: number) => `${(bytes / 1_048_576).toFixed(0)} MB`

type Asset = { name: string; size: number; browser_download_url: string; download_count: number }
type GithubRelease = { tag_name: string; html_url: string; prerelease: boolean; draft: boolean; assets: Asset[] }

function countDownloads(releases: GithubRelease[]): Downloads {
  const d = { total: 0, mac: 0, windows: 0, linux: 0 }
  for (const a of releases.flatMap((r) => r.assets)) {
    const os = a.name.endsWith('-macos.zip') ? 'mac' : a.name.endsWith('-windows.zip') ? 'windows' : /-linux-.+\.tar\.gz$/.test(a.name) ? 'linux' : null
    if (!os) continue
    d[os] += a.download_count
    d.total += a.download_count
  }
  return d
}

/** The newest release from GitHub, so the buttons point at the real files, and how often every release was downloaded.
 * Null until it arrives, or if it cannot. */
export function useRelease(): Release | null {
  const [release, setRelease] = useState<Release | null>(null)
  useEffect(() => {
    const ctl = new AbortController()
    fetch(`https://api.github.com/repos/${REPO.split('github.com/')[1]}/releases?per_page=100`, { signal: ctl.signal })
      .then((r) => (r.ok ? r.json() : Promise.reject(r.status)))
      .then((all: GithubRelease[]) => {
        const published = all.filter((r) => !r.draft)
        const j = published.find((r) => !r.prerelease)
        if (!j) return
        const pick = (suffix: string): Download | undefined => {
          const a = j.assets.find((x) => x.name.endsWith(suffix))
          return a && { url: a.browser_download_url, size: mb(a.size) }
        }
        setRelease({
          version: j.tag_name.replace(/^v/, ''),
          page: j.html_url || RELEASES,
          downloads: countDownloads(published),
          mac: pick('-macos.zip'),
          windows: pick('-windows.zip'),
          linux: { x86_64: pick('-linux-x86_64.tar.gz'), aarch64: pick('-linux-aarch64.tar.gz') },
        })
      })
      .catch(() => {})
    return () => ctl.abort()
  }, [])
  return release
}

export type Platform = 'mac' | 'windows' | 'linux' | 'other'

export function detectPlatform(): Platform {
  if (typeof navigator === 'undefined') return 'other'
  const s = `${navigator.userAgent} ${navigator.platform ?? ''}`
  if (/Mac/i.test(s) && !/iPhone|iPad/i.test(s)) return 'mac'
  if (/Win/i.test(s)) return 'windows'
  // Android phones say Linux too, and Handset does not run on them
  if (/Linux|X11/i.test(s) && !/Android/i.test(s)) return 'linux'
  return 'other'
}

/** The visitor's CPU on Linux. Firefox puts it in the user agent; Chrome always says x86_64 there but can be asked. */
export function useLinuxArch(): LinuxArch {
  const ua = typeof navigator === 'undefined' ? '' : navigator.userAgent
  const [arch, setArch] = useState<LinuxArch>(/aarch64|arm64|armv8/i.test(ua) ? 'aarch64' : 'x86_64')
  useEffect(() => {
    const data = (navigator as { userAgentData?: { getHighEntropyValues(h: string[]): Promise<{ architecture?: string }> } })
      .userAgentData
    data
      ?.getHighEntropyValues(['architecture'])
      .then((v) => v.architecture === 'arm' && setArch('aarch64'))
      .catch(() => {})
  }, [])
  return arch
}
