import { useEffect, useState } from 'react'
import { REPO, RELEASES } from './links'

export type Download = { url: string; size: string }
export type LinuxArch = 'x86_64' | 'aarch64'
export type Release = {
  version: string
  page: string
  mac?: Download
  windows?: Download
  linux: Partial<Record<LinuxArch, Download>>
}

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
