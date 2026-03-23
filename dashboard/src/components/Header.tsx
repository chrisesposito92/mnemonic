import { useState, useEffect } from 'preact/hooks'
import { fetchHealth, type HealthResponse } from '../api'

type HeaderState =
  | { kind: 'loading' }
  | { kind: 'loaded'; data: HealthResponse }
  | { kind: 'error' }

interface HeaderProps {
  token: string | null
}

export default function Header({ token: _token }: HeaderProps) {
  const [state, setState] = useState<HeaderState>({ kind: 'loading' })

  useEffect(() => {
    let cancelled = false
    const controller = new AbortController()

    const poll = () => {
      fetchHealth(controller.signal)
        .then(data => { if (!cancelled) setState({ kind: 'loaded', data }) })
        .catch(() => { if (!cancelled) setState({ kind: 'error' }) })
    }

    poll()
    const interval = setInterval(poll, 30_000)

    return () => {
      cancelled = true
      controller.abort()
      clearInterval(interval)
    }
  }, []) // token NOT in deps -- fetchHealth is always public

  const dotColor =
    state.kind === 'loading' ? 'var(--color-border)' :
    state.kind === 'loaded' && state.data.status === 'ok' ? 'var(--color-accent)' :
    'var(--color-error)'

  const backendName = state.kind === 'loaded' ? state.data.backend : ''

  return (
    <div style={{
      display: 'flex',
      alignItems: 'center',
      gap: '12px',
      padding: '16px 24px',
      borderBottom: '1px solid var(--color-border)',
    }}>
      <span style={{
        fontSize: '20px',
        fontWeight: 600,
        lineHeight: 1.2,
        color: 'var(--color-text)',
      }}>
        Mnemonic
      </span>
      <span style={{
        width: '12px',
        height: '12px',
        borderRadius: '50%',
        display: 'inline-block',
        background: dotColor,
        flexShrink: 0,
      }} />
      {backendName && (
        <span style={{
          fontSize: '12px',
          fontWeight: 400,
          lineHeight: 1.4,
          color: 'var(--color-text-muted)',
        }}>
          {backendName}
        </span>
      )}
    </div>
  )
}
