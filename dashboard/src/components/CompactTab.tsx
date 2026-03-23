import { useState, useEffect, useRef } from 'preact/hooks'
import {
  compactMemories,
  fetchMemoryById,
  fetchStats,
  UnauthorizedError,
  type CompactResponse,
  type Memory,
} from '../api'
import ClusterPreview from './ClusterPreview'
import SkeletonRows from './SkeletonRows'
import ErrorMessage from './ErrorMessage'

interface CompactTabProps {
  token: string | null
  onUnauthorized: () => void
}

type CompactState =
  | { kind: 'idle' }
  | { kind: 'loading-dry-run' }
  | { kind: 'preview'; result: CompactResponse; memories: Map<string, Memory> }
  | { kind: 'loading-execute' }
  | { kind: 'success'; result: CompactResponse }
  | { kind: 'empty' }
  | { kind: 'error'; message: string }

function isValidThreshold(value: string): boolean {
  const n = parseFloat(value)
  return !isNaN(n) && isFinite(n) && n >= 0 && n <= 1
}

const CHUNK_SIZE = 5

export default function CompactTab({ token, onUnauthorized }: CompactTabProps) {
  const [state, setState] = useState<CompactState>({ kind: 'idle' })
  const [agents, setAgents] = useState<string[]>([])
  const [agentsLoading, setAgentsLoading] = useState(true)
  const [selectedAgent, setSelectedAgent] = useState('')
  const [threshold, setThreshold] = useState('0.85')
  const dryRunControllerRef = useRef<AbortController | null>(null)

  // On mount: fetch agents from /stats for dropdown population
  useEffect(() => {
    const controller = new AbortController()
    setAgentsLoading(true)

    fetchStats(token, controller.signal)
      .then(data => {
        const agentOptions = data.agents.map(a => a.agent_id === '' ? '__none__' : a.agent_id)
        setAgents(agentOptions)
        setAgentsLoading(false)
      })
      .catch(err => {
        if (err instanceof UnauthorizedError) {
          onUnauthorized()
          return
        }
        if (err.name === 'AbortError') return
        // Non-critical -- dropdown just won't populate
        setAgentsLoading(false)
      })

    return () => controller.abort()
  }, [token, onUnauthorized])

  // Cleanup on unmount: abort any in-flight dry-run or memory fetches
  useEffect(() => {
    return () => {
      if (dryRunControllerRef.current) {
        dryRunControllerRef.current.abort()
      }
    }
  }, [])

  // Preview invalidation: if agent or threshold changes while preview is showing, discard it
  useEffect(() => {
    if (state.kind === 'preview') {
      // Agent or threshold changed while preview is showing -- invalidate
      if (dryRunControllerRef.current) {
        dryRunControllerRef.current.abort()
        dryRunControllerRef.current = null
      }
      setState({ kind: 'idle' })
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedAgent, threshold])

  async function handleDryRun() {
    const t = parseFloat(threshold)
    if (isNaN(t) || !isFinite(t) || t < 0 || t > 1) {
      setState({ kind: 'error', message: 'Threshold must be a number between 0 and 1.' })
      return
    }

    const apiAgentId = selectedAgent === '__none__' ? '' : selectedAgent

    setState({ kind: 'loading-dry-run' })

    const controller = new AbortController()
    dryRunControllerRef.current = controller

    try {
      const result = await compactMemories(
        token,
        { agent_id: apiAgentId, threshold: t, dry_run: true },
        controller.signal,
      )

      if (result.clusters_found === 0) {
        if (!controller.signal.aborted) {
          setState({ kind: 'empty' })
        }
        return
      }

      // Collect all unique source IDs from all clusters
      const allIds = result.id_mapping.flatMap(c => c.source_ids)
      const uniqueIds = [...new Set(allIds)]

      // Fetch source memories with concurrency limiting and graceful degradation
      const memoryMap = new Map<string, Memory>()
      for (let i = 0; i < uniqueIds.length; i += CHUNK_SIZE) {
        if (controller.signal.aborted) break
        const chunk = uniqueIds.slice(i, i + CHUNK_SIZE)
        // Use Promise.allSettled for partial-failure handling
        const results = await Promise.allSettled(
          chunk.map(id => fetchMemoryById(token, id, controller.signal))
        )
        for (let j = 0; j < results.length; j++) {
          const r = results[j]
          if (r.status === 'fulfilled') {
            memoryMap.set(chunk[j], r.value)
          }
          // 'rejected' entries are silently skipped -- ClusterPreview
          // falls back to showing the raw memory ID
        }
      }

      // Prevent stale state update after abort
      if (!controller.signal.aborted) {
        setState({ kind: 'preview', result, memories: memoryMap })
      }
    } catch (err: unknown) {
      if (err instanceof UnauthorizedError) {
        onUnauthorized()
        return
      }
      if ((err as Error).name === 'AbortError') return
      setState({ kind: 'error', message: 'Compaction failed. Check server logs and try again.' })
    }
  }

  async function handleExecute() {
    setState({ kind: 'loading-execute' })

    const controller = new AbortController()
    const apiAgentId = selectedAgent === '__none__' ? '' : selectedAgent

    try {
      const result = await compactMemories(
        token,
        { agent_id: apiAgentId, threshold: parseFloat(threshold), dry_run: false },
        controller.signal,
      )
      setState({ kind: 'success', result })
    } catch (err: unknown) {
      if (err instanceof UnauthorizedError) {
        onUnauthorized()
        return
      }
      if ((err as Error).name === 'AbortError') return
      setState({ kind: 'error', message: 'Compaction failed. Check server logs and try again.' })
    }
  }

  function handleDiscard() {
    if (dryRunControllerRef.current) {
      dryRunControllerRef.current.abort()
      dryRunControllerRef.current = null
    }
    setState({ kind: 'idle' })
  }

  function handleReset() {
    setState({ kind: 'idle' })
  }

  const isRunning = state.kind === 'loading-dry-run' || state.kind === 'loading-execute'
  const runDisabled = !selectedAgent || !isValidThreshold(threshold) || isRunning

  const selectStyle = {
    padding: '4px 8px',
    fontSize: '14px',
    fontFamily: 'inherit',
    background: 'var(--color-surface)',
    color: 'var(--color-text)',
    border: '1px solid var(--color-border)',
    borderRadius: '4px',
  }

  const primaryButtonStyle = {
    padding: '8px 16px',
    fontSize: '14px',
    fontWeight: 600,
    fontFamily: 'inherit',
    background: 'var(--color-accent)',
    color: 'var(--color-bg)',
    border: 'none',
    borderRadius: '4px',
    cursor: runDisabled ? 'default' : 'pointer',
    opacity: runDisabled ? 0.6 : 1,
  }

  // No agents available state
  if (!agentsLoading && agents.length === 0) {
    return (
      <div style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
        <div style={{ textAlign: 'center', padding: '48px 0' }}>
          <div style={{ fontSize: '14px', fontWeight: 400, color: 'var(--color-text)' }}>
            No agents available
          </div>
          <div style={{ fontSize: '12px', fontWeight: 400, color: 'var(--color-text-muted)', marginTop: '8px' }}>
            Agents will appear here once memories are stored.
          </div>
        </div>
      </div>
    )
  }

  // Controls row (rendered in all states except no-agents)
  const controlsRow = (
    <div style={{ display: 'flex', gap: '16px', alignItems: 'center', flexWrap: 'wrap' }}>
      <select
        value={selectedAgent}
        onChange={(e) => setSelectedAgent((e.target as HTMLSelectElement).value)}
        style={selectStyle}
      >
        <option value="" disabled>Select agent...</option>
        {agents.map(agent => (
          <option key={agent} value={agent}>
            {agent === '__none__' ? '(none)' : agent}
          </option>
        ))}
      </select>

      <div style={{ display: 'flex', gap: '4px', alignItems: 'center' }}>
        <span style={{ fontSize: '12px', fontWeight: 400, color: 'var(--color-text-muted)' }}>
          threshold:
        </span>
        <input
          type="number"
          step="0.01"
          min="0"
          max="1"
          value={threshold}
          onInput={(e) => setThreshold((e.target as HTMLInputElement).value)}
          style={{
            ...selectStyle,
            width: '70px',
          }}
        />
      </div>

      <button
        onClick={handleDryRun}
        disabled={runDisabled}
        style={primaryButtonStyle}
      >
        Run Dry Run
      </button>
    </div>
  )

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
      {/* Controls row always visible */}
      {controlsRow}

      {/* State-specific content */}
      {state.kind === 'loading-dry-run' && (
        <SkeletonRows rows={5} />
      )}

      {state.kind === 'preview' && (
        <div>
          {/* Summary line */}
          <div style={{ fontSize: '14px', fontWeight: 600, color: 'var(--color-text)', marginBottom: '16px' }}>
            {state.result.clusters_found} clusters found, {state.result.memories_merged} memories -&gt; {state.result.id_mapping.length} compacted
          </div>

          {/* Truncation warning */}
          {state.result.truncated && (
            <div style={{ fontSize: '12px', fontWeight: 400, color: 'var(--color-error)', marginBottom: '8px' }}>
              Results may be incomplete. The server capped results at the maximum candidate limit. Some compactable clusters may not be shown.
            </div>
          )}

          {/* Cluster table */}
          <div>
            {state.result.id_mapping.map((cluster, idx) => (
              <ClusterPreview
                key={idx}
                clusterIndex={idx}
                sourceIds={cluster.source_ids}
                memories={state.memories}
              />
            ))}
          </div>

          {/* Confirm / Discard buttons */}
          <div style={{ display: 'flex', gap: '8px', marginTop: '16px' }}>
            <button
              onClick={handleExecute}
              style={{
                padding: '8px 16px',
                fontSize: '14px',
                fontWeight: 600,
                fontFamily: 'inherit',
                background: 'var(--color-accent)',
                color: 'var(--color-bg)',
                border: 'none',
                borderRadius: '4px',
                cursor: 'pointer',
              }}
            >
              Confirm Compact
            </button>
            <button
              onClick={handleDiscard}
              style={{
                padding: '8px 16px',
                fontSize: '14px',
                fontWeight: 400,
                fontFamily: 'inherit',
                background: 'transparent',
                color: 'var(--color-text-muted)',
                border: '1px solid var(--color-border)',
                borderRadius: '4px',
                cursor: 'pointer',
              }}
            >
              Discard Preview
            </button>
          </div>
        </div>
      )}

      {state.kind === 'loading-execute' && (
        <SkeletonRows rows={3} />
      )}

      {state.kind === 'success' && (
        <div>
          <div style={{ fontSize: '14px', fontWeight: 400, color: 'var(--color-text)' }}>
            Compaction complete. {state.result.clusters_found} clusters merged {state.result.memories_merged} memories into {state.result.memories_created}.
          </div>
          <div
            onClick={handleReset}
            style={{
              fontSize: '14px',
              fontWeight: 400,
              color: 'var(--color-accent)',
              cursor: 'pointer',
              textDecoration: 'underline',
              marginTop: '8px',
            }}
          >
            Run another compaction
          </div>
        </div>
      )}

      {state.kind === 'empty' && (
        <div style={{ textAlign: 'center', padding: '48px 0' }}>
          <div style={{ fontSize: '14px', fontWeight: 400, color: 'var(--color-text)' }}>
            No compactable clusters found
          </div>
          <div style={{ fontSize: '12px', fontWeight: 400, color: 'var(--color-text-muted)', marginTop: '8px' }}>
            All memories for this agent are already distinct enough at the current threshold. Try lowering the threshold for more aggressive clustering.
          </div>
          <div
            onClick={handleReset}
            style={{
              fontSize: '14px',
              fontWeight: 400,
              color: 'var(--color-accent)',
              cursor: 'pointer',
              textDecoration: 'underline',
              marginTop: '16px',
            }}
          >
            Try again
          </div>
        </div>
      )}

      {state.kind === 'error' && (
        <div>
          <ErrorMessage message={state.message} />
          <div
            onClick={handleReset}
            style={{
              fontSize: '14px',
              fontWeight: 400,
              color: 'var(--color-accent)',
              cursor: 'pointer',
              textDecoration: 'underline',
              marginTop: '8px',
            }}
          >
            Try again
          </div>
        </div>
      )}
    </div>
  )
}
