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
    <div className="mt-2 flex min-w-0 items-start justify-between gap-3 border-2 border-ink bg-ink py-3 pl-4 pr-3">
      <pre className="min-w-0 flex-1 overflow-x-auto whitespace-pre font-mono text-[0.78rem] leading-relaxed text-paper">{text}</pre>
      <button
        type="button"
        onClick={copy}
        className="label shrink-0 border-2 border-paper px-2.5 py-1 text-paper transition-colors hover:bg-marker hover:text-ink"
        aria-live="polite"
      >
        {copied ? 'Copied' : 'Copy'}
      </button>
    </div>
  )
}
