import { useState } from 'react'

export function CopyBlock({ text }: { text: string }) {
  const [copied, setCopied] = useState(false)
  const copy = () => {
    navigator.clipboard?.writeText(text).then(() => {
      setCopied(true)
      setTimeout(() => setCopied(false), 1600)
    })
  }
  return (
    <div className="mt-2 flex min-w-0 items-start justify-between gap-3 rounded-xl bg-black/40 py-3 pl-4 pr-3 ring-1 ring-stage-line">
      <pre className="min-w-0 flex-1 overflow-x-auto whitespace-pre text-sm leading-relaxed text-paper">{text}</pre>
      <button
        type="button"
        onClick={copy}
        className="shrink-0 rounded-lg px-2.5 py-1 text-sm font-medium text-stage-text transition-colors hover:bg-stage-2 hover:text-paper"
        aria-live="polite"
      >
        {copied ? 'Copied' : 'Copy'}
      </button>
    </div>
  )
}
