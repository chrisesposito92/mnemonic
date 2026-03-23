import { useState, useEffect, useCallback } from 'preact/hooks'
import { fetchMemories, fetchStats, UnauthorizedError, type Memory } from '../api'
import MemoryRow from './MemoryRow'
import FilterBar from './FilterBar'
import Pagination from './Pagination'
import SkeletonRows from './SkeletonRows'
import ErrorMessage from './ErrorMessage'

interface MemoriesTabProps {
  token: string | null
  onUnauthorized: () => void
}

type TabState =
  | { kind: 'loading' }
  | { kind: 'loaded'; memories: Memory[]; total: number }
  | { kind: 'empty' }
  | { kind: 'error'; message: string }

const PAGE_SIZE = 20

export default function MemoriesTab({ token, onUnauthorized }: MemoriesTabProps) {
  const [state, setState] = useState<TabState>({ kind: 'loading' })
  const [offset, setOffset] = useState(0)

  // Filter state
  const [filterAgent, setFilterAgent] = useState('')
  const [filterSession, setFilterSession] = useState('')
  const [filterTag, setFilterTag] = useState('')

  // Agent list from /stats for dropdown population (review concern #1)
  const [agentOptions, setAgentOptions] = useState<string[]>([])

  // Session + tag lists derived from current page response
  const [sessionOptions, setSessionOptions] = useState<string[]>([])
  const [tagOptions, setTagOptions] = useState<string[]>([])

  // Fetch agent list from /stats on mount (review concern #1)
  useEffect(() => {
    const controller = new AbortController()

    fetchStats(token, controller.signal)
      .then(data => {
        const agents = data.agents.map(a => a.agent_id)
        setAgentOptions(agents)
      })
      .catch(err => {
        if (err instanceof UnauthorizedError) {
          onUnauthorized()
          return
        }
        // Non-critical: filter dropdown just won't have pre-populated agents
      })

    return () => controller.abort()
  }, [token, onUnauthorized])

  // Fetch memories when offset or filters change
  useEffect(() => {
    const controller = new AbortController()
    setState({ kind: 'loading' })

    fetchMemories(token, {
      limit: PAGE_SIZE,
      offset,
      agent_id: filterAgent || undefined,
      session_id: filterSession || undefined,
      tag: filterTag || undefined,
    }, controller.signal)
      .then(data => {
        if (data.memories.length === 0 && data.total === 0) {
          setState({ kind: 'empty' })
        } else {
          setState({ kind: 'loaded', memories: data.memories, total: data.total })

          // Derive session and tag options from current response
          const sessions = [...new Set(data.memories.map(m => m.session_id).filter(Boolean))]
          const tags = [...new Set(data.memories.flatMap(m => m.tags))]
          setSessionOptions(prev => {
            const merged = [...new Set([...prev, ...sessions])]
            return merged.sort()
          })
          setTagOptions(prev => {
            const merged = [...new Set([...prev, ...tags])]
            return merged.sort()
          })
        }
      })
      .catch(err => {
        if (err instanceof UnauthorizedError) {
          onUnauthorized()
          return
        }
        if (err.name === 'AbortError') return // Stale request cancelled -- ignore
        setState({ kind: 'error', message: 'Failed to load data. Reload the page or check server status.' })
      })

    return () => controller.abort() // Cancel stale request (review concern #6)
  }, [token, offset, filterAgent, filterSession, filterTag, onUnauthorized])

  // Filter change handlers -- all reset offset to 0 (research Pitfall 2)
  const handleAgentChange = useCallback((value: string) => {
    setFilterAgent(value)
    setOffset(0)
  }, [])

  const handleSessionChange = useCallback((value: string) => {
    setFilterSession(value)
    setOffset(0)
  }, [])

  const handleTagChange = useCallback((value: string) => {
    setFilterTag(value)
    setOffset(0)
  }, [])

  const handleClearFilters = useCallback(() => {
    setFilterAgent('')
    setFilterSession('')
    setFilterTag('')
    setOffset(0)
  }, [])

  // Table header columns per D-05
  const columns = ['Content', 'Agent', 'Session', 'Tags', 'Created']

  const headerCellStyle = {
    padding: '8px 12px',
    fontSize: '12px',
    fontWeight: 400,
    lineHeight: 1.4,
    color: 'var(--color-text-muted)',
    textAlign: 'left' as const,
    borderBottom: '1px solid var(--color-border)',
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
      <FilterBar
        agents={agentOptions}
        sessions={sessionOptions}
        tags={tagOptions}
        selectedAgent={filterAgent}
        selectedSession={filterSession}
        selectedTag={filterTag}
        onAgentChange={handleAgentChange}
        onSessionChange={handleSessionChange}
        onTagChange={handleTagChange}
        onClear={handleClearFilters}
      />

      {state.kind === 'loading' && <SkeletonRows rows={5} />}

      {state.kind === 'error' && <ErrorMessage message={state.message} />}

      {state.kind === 'empty' && (
        <div style={{ textAlign: 'center', padding: '48px 0' }}>
          <div style={{ fontSize: '14px', fontWeight: 400, color: 'var(--color-text)' }}>
            No memories stored
          </div>
          <div style={{ fontSize: '12px', fontWeight: 400, color: 'var(--color-text-muted)', marginTop: '8px' }}>
            Memories will appear here once agents start storing them.
          </div>
        </div>
      )}

      {state.kind === 'loaded' && (
        <>
          <table style={{
            width: '100%',
            borderCollapse: 'collapse',
            tableLayout: 'auto',
          }}>
            <thead>
              <tr>
                {columns.map(col => (
                  <th key={col} style={headerCellStyle}>{col}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {state.memories.map(memory => (
                <MemoryRow key={memory.id} memory={memory} />
              ))}
            </tbody>
          </table>
          <Pagination
            offset={offset}
            limit={PAGE_SIZE}
            total={state.total}
            onPrev={() => setOffset(Math.max(0, offset - PAGE_SIZE))}
            onNext={() => setOffset(offset + PAGE_SIZE)}
          />
        </>
      )}
    </div>
  )
}
