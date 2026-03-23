import { useState, useEffect, useCallback } from 'preact/hooks'
import { searchMemories, fetchStats, UnauthorizedError, type SearchResultItem } from '../api'
import DistanceBar from './DistanceBar'
import SkeletonRows from './SkeletonRows'
import ErrorMessage from './ErrorMessage'

interface SearchTabProps {
  token: string | null
  onUnauthorized: () => void
}

type SearchState =
  | { kind: 'idle' }     // No search performed yet
  | { kind: 'loading' }
  | { kind: 'loaded'; results: SearchResultItem[] }
  | { kind: 'empty' }
  | { kind: 'error'; message: string }

function truncate(s: string, len: number): string {
  return s.length > len ? s.slice(0, len) + '...' : s
}

export default function SearchTab({ token, onUnauthorized }: SearchTabProps) {
  const [state, setState] = useState<SearchState>({ kind: 'idle' })
  const [query, setQuery] = useState('')
  const [filterAgent, setFilterAgent] = useState('')
  const [filterTag, setFilterTag] = useState('')
  const [agentOptions, setAgentOptions] = useState<string[]>([])

  // Fetch agent list from /stats for filter dropdown (review concern #1)
  useEffect(() => {
    const controller = new AbortController()

    fetchStats(token, controller.signal)
      .then(data => setAgentOptions(data.agents.map(a => a.agent_id)))
      .catch(err => {
        if (err instanceof UnauthorizedError) onUnauthorized()
      })

    return () => controller.abort()
  }, [token, onUnauthorized])

  const doSearch = useCallback(() => {
    const q = query.trim()
    if (!q) return

    const controller = new AbortController()
    setState({ kind: 'loading' })

    searchMemories(token, {
      q,
      agent_id: filterAgent || undefined,
      tag: filterTag || undefined,
      limit: 20,
    }, controller.signal)
      .then(data => {
        if (data.memories.length === 0) {
          setState({ kind: 'empty' })
        } else {
          setState({ kind: 'loaded', results: data.memories })
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

    // Note: cleanup for this specific search is not in useEffect,
    // so the AbortController lives for the lifetime of the search.
    // For rapid re-searches, the state check in .then handles staleness.
  }, [query, filterAgent, filterTag, token, onUnauthorized])

  const handleKeyDown = (e: KeyboardEvent) => {
    if (e.key === 'Enter') {
      e.preventDefault()
      doSearch()
    }
  }

  const selectStyle = {
    padding: '6px 8px',
    fontSize: '12px',
    fontWeight: 400,
    fontFamily: 'inherit',
    background: 'var(--color-surface)',
    border: '1px solid var(--color-border)',
    borderRadius: '4px',
    color: 'var(--color-text)',
    outline: 'none',
    minWidth: '120px',
  }

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
    verticalAlign: 'top' as const,
  }

  const mutedCellStyle = {
    ...cellStyle,
    fontSize: '12px',
    color: 'var(--color-text-muted)',
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
      {/* Search input row */}
      <div style={{ display: 'flex', gap: '8px', alignItems: 'center', flexWrap: 'wrap' }}>
        <input
          type="text"
          placeholder="Search memories..."
          value={query}
          onInput={(e) => setQuery((e.target as HTMLInputElement).value)}
          onKeyDown={handleKeyDown}
          style={{
            flex: 1,
            minWidth: '200px',
            padding: '8px 12px',
            fontSize: '14px',
            fontWeight: 400,
            fontFamily: 'inherit',
            background: 'var(--color-surface)',
            border: '1px solid var(--color-border)',
            borderRadius: '4px',
            color: 'var(--color-text)',
            outline: 'none',
          }}
        />
        <button
          onClick={doSearch}
          disabled={!query.trim() || state.kind === 'loading'}
          style={{
            padding: '8px 16px',
            fontSize: '14px',
            fontWeight: 600,
            fontFamily: 'inherit',
            background: 'var(--color-accent)',
            color: 'var(--color-bg)',
            border: 'none',
            borderRadius: '4px',
            cursor: !query.trim() || state.kind === 'loading' ? 'default' : 'pointer',
            opacity: !query.trim() || state.kind === 'loading' ? 0.6 : 1,
          }}
        >
          Search Memories
        </button>
      </div>

      {/* Optional filters */}
      <div style={{ display: 'flex', gap: '16px', alignItems: 'center' }}>
        <div style={{ display: 'flex', gap: '4px', alignItems: 'center' }}>
          <span style={{ fontSize: '12px', color: 'var(--color-text-muted)' }}>agent:</span>
          <select value={filterAgent} onChange={(e) => setFilterAgent((e.target as HTMLSelectElement).value)} style={selectStyle}>
            <option value="">all</option>
            {agentOptions.map(a => <option key={a} value={a}>{a || '(none)'}</option>)}
          </select>
        </div>
        <div style={{ display: 'flex', gap: '4px', alignItems: 'center' }}>
          <span style={{ fontSize: '12px', color: 'var(--color-text-muted)' }}>tag:</span>
          <input
            type="text"
            placeholder="filter by tag"
            value={filterTag}
            onInput={(e) => setFilterTag((e.target as HTMLInputElement).value)}
            style={{ ...selectStyle, minWidth: '140px' }}
          />
        </div>
      </div>

      {/* Results */}
      {state.kind === 'idle' && (
        <div style={{ color: 'var(--color-text-muted)', fontSize: '14px', padding: '24px 0' }}>
          Enter a query and press Enter or click "Search Memories".
        </div>
      )}

      {state.kind === 'loading' && <SkeletonRows rows={5} />}

      {state.kind === 'error' && <ErrorMessage message={state.message} />}

      {state.kind === 'empty' && (
        <div style={{ textAlign: 'center', padding: '48px 0' }}>
          <div style={{ fontSize: '14px', fontWeight: 400, color: 'var(--color-text)' }}>
            Nothing matched your search
          </div>
          <div style={{ fontSize: '12px', fontWeight: 400, color: 'var(--color-text-muted)', marginTop: '8px' }}>
            No memories matched your query. Try different search terms or broaden your filters.
          </div>
        </div>
      )}

      {state.kind === 'loaded' && (
        <table style={{ width: '100%', borderCollapse: 'collapse' }}>
          <thead>
            <tr>
              <th style={headerCellStyle}>Similarity</th>
              <th style={headerCellStyle}>Content</th>
              <th style={headerCellStyle}>Agent</th>
              <th style={headerCellStyle}>Session</th>
            </tr>
          </thead>
          <tbody>
            {state.results.map(item => (
              <tr key={item.id} style={{ borderBottom: '1px solid var(--color-border)' }}>
                <td style={cellStyle}>
                  <DistanceBar distance={item.distance} />
                </td>
                <td style={cellStyle}>{truncate(item.content, 80)}</td>
                <td style={mutedCellStyle}>{item.agent_id || '\u2014'}</td>
                <td style={mutedCellStyle}>{item.session_id || '\u2014'}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  )
}
