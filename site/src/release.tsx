import { createContext, useContext, type ReactNode } from 'react'
import { detectPlatform, useRelease, type Platform, type Release } from './useRelease'

type Value = { release: Release | null; platform: Platform }
const Ctx = createContext<Value>({ release: null, platform: 'other' })

/** The latest release and the visitor's system, fetched once for the whole site. */
export function ReleaseProvider({ children }: { children: ReactNode }) {
  const release = useRelease()
  return <Ctx.Provider value={{ release, platform: detectPlatform() }}>{children}</Ctx.Provider>
}

export const useReleaseInfo = () => useContext(Ctx)
