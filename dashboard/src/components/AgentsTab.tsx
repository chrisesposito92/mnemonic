import { useState, useEffect } from 'preact/hooks'
import { fetchStats, UnauthorizedError, type AgentStats } from '../api'
import SkeletonRows from './SkeletonRows'
import ErrorMessage from './ErrorMessage'

interface AgentsTabProps {
  token: string | null
  onUnauthorized: () => void
}

type TabState =
  | { kind: 'loading' }
  | { kind: 'loaded'; agents: AgentStats[] }
  | { kind: 'empty' }
  | { kind: 'error'; message: string }

function relativeTime(iso: string): string {
  const diff = Math.floor((Date.now() - new Date(iso).getTime()) / 1000)
  if (diff < 0) return 'just now'
  if (diff < 60) return `${diff}s ago`
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`
  return `${Math.floor(diff / 86400)}d ago`
}

export default function AgentsTab({ token, onUnauthorized }: AgentsTabProps) {
  const [state, setState] = useState<TabState>({ kind: 'loading' })

  useEffect(() => {
    const controller = new AbortController()
    setState({ kind: 'loading' })

    fetchStats(token, controller.signal)
      .then(data => {
        if (data.agents.length === 0) {
          setState({ kind: 'empty' })
        } else {
          setState({ kind: 'loaded', agents: data.agents })
        }
      })
      .catch(err => {
        if (err instanceof UnauthorizedError) {
          onUnauthorized()
          return
        }
        if (err.name === 'AbortError') return
        setState({ kind: 'error', message: 'Failed to load data. Reload the page or check server status.' })
      })

    return () => controller.abort()
  }, [token, onUnauthorized])

  const headerCellStyle = {
    padding: '8px 12px',
    fontSize: '12px',
    fontWeight: 400,
    lineHeight: 1.4,
    color: 'var(--color-text-muted)',
    textAlign: 'left' as const,
    borderBottom: '1px solid var(--color-border)',
  }

  const cellStyle = {
    padding: '8px 12px',
    fontSize: '14px',
    fontWeight: 400,
    lineHeight: 1.5,
    color: 'var(--color-text)',
  }

  const mutedCellStyle = {
    ...cellStyle,
    fontSize: '12px',
    color: 'var(--color-text-muted)',
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
      {state.kind === 'loading' && <SkeletonRows rows={3} />}

      {state.kind === 'error' && <ErrorMessage message={state.message} />}

      {state.kind === 'empty' && (
        <div style={{ textAlign: 'center', padding: '48px 0' }}>
          <div style={{ fontSize: '14px', fontWeight: 400, color: 'var(--color-text)' }}>
            No agents found
          </div>
          <div style={{ fontSize: '12px', fontWeight: 400, color: 'var(--color-text-muted)', marginTop: '8px' }}>
            Agent breakdowns will appear here once memories are stored.
          </div>
        </div>
      )}

      {state.kind === 'loaded' && (
        <table style={{ width: '100%', borderCollapse: 'collapse' }}>
          <thead>
            <tr>
              <th style={headerCellStyle}>Agent</th>
              <th style={headerCellStyle}>Memories</th>
              <th style={headerCellStyle}>Last Active</th>
            </tr>
          </thead>
          <tbody>
            {state.agents.map(agent => (
              <tr key={agent.agent_id} style={{ borderBottom: '1px solid var(--color-border)' }}>
                {/* Empty agent_id displayed as "(none)" per research Pitfall 5 -- not a blank cell */}
                <td style={cellStyle}>{agent.agent_id || '(none)'}</td>
                <td style={cellStyle}>{agent.memory_count}</td>
                <td style={mutedCellStyle}>{agent.last_active ? relativeTime(agent.last_active) : '\u2014'}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  )
}
