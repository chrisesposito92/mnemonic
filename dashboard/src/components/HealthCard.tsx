import { useEffect, useState } from 'preact/hooks'

interface HealthResponse {
  status: string
  backend: string
  version?: string
}

type CardState =
  | { kind: 'loading' }
  | { kind: 'loaded'; data: HealthResponse }
  | { kind: 'error'; message: string }

const HEALTH_TIMEOUT_MS = 10_000

export default function HealthCard() {
  const [state, setState] = useState<CardState>({ kind: 'loading' })

  useEffect(() => {
    fetch('/health', { signal: AbortSignal.timeout(HEALTH_TIMEOUT_MS) })
      .then((res) => {
        if (!res.ok) throw new Error(`HTTP ${res.status}`)
        return res.json()
      })
      .then((data: HealthResponse) => setState({ kind: 'loaded', data }))
      .catch((err) => {
        const message = err.name === 'TimeoutError'
          ? 'GET /health timed out. Check that the server is running and reload the page.'
          : 'GET /health failed. Check that the server is running and reload the page.'
        setState({ kind: 'error', message })
      })
  }, [])

  return (
    <div
      class="w-full max-w-sm"
      style={{
        background: 'var(--color-surface)',
        border: '1px solid var(--color-border)',
        borderRadius: '8px',
        padding: '24px',
      }}
    >
      <div
        style={{ fontSize: '12px', fontWeight: 400, lineHeight: 1.4, color: 'var(--color-text-muted)', marginBottom: '16px' }}
      >
        Health
      </div>

      {state.kind === 'loading' && (
        <div class="flex flex-col gap-2">
          {[1, 2, 3].map((i) => (
            <div
              key={i}
              style={{
                height: '12px',
                background: 'var(--color-border)',
                opacity: 0.4,
                borderRadius: '2px',
              }}
            />
          ))}
        </div>
      )}

      {state.kind === 'loaded' && (
        <div class="flex flex-col" style={{ gap: '8px' }}>
          <Row label="status:">
            <span style={{ display: 'inline-flex', alignItems: 'center', gap: '8px' }}>
              {state.data.status}
              <span
                style={{
                  width: '12px',
                  height: '12px',
                  borderRadius: '50%',
                  display: 'inline-block',
                  background:
                    state.data.status === 'ok'
                      ? 'var(--color-accent)'
                      : 'var(--color-error)',
                }}
              />
            </span>
          </Row>
          <Row label="backend:">{state.data.backend}</Row>
          <Row label="version:">{state.data.version ?? '--'}</Row>
        </div>
      )}

      {state.kind === 'error' && (
        <div>
          <div style={{ fontSize: '14px', fontWeight: 400, color: 'var(--color-error)' }}>
            Could not reach API
          </div>
          <div
            style={{
              fontSize: '14px',
              fontWeight: 400,
              color: 'var(--color-error)',
              marginTop: '4px',
            }}
          >
            {state.message}
          </div>
        </div>
      )}
    </div>
  )
}

function Row({ label, children }: { label: string; children: preact.ComponentChildren }) {
  return (
    <div style={{ display: 'flex', gap: '8px', alignItems: 'baseline' }}>
      <span style={{ fontSize: '12px', fontWeight: 400, lineHeight: 1.4, color: 'var(--color-text-muted)' }}>
        {label}
      </span>
      <span style={{ fontSize: '14px', fontWeight: 400, lineHeight: 1.5, color: 'var(--color-text)' }}>
        {children}
      </span>
    </div>
  )
}
