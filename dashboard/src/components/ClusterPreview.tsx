import type { Memory } from '../api'

interface ClusterPreviewProps {
  clusterIndex: number
  sourceIds: string[]
  memories: Map<string, Memory>
}

function truncate(s: string, len: number): string {
  return s.length > len ? s.slice(0, len) + '...' : s
}

export default function ClusterPreview({ clusterIndex, sourceIds, memories }: ClusterPreviewProps) {
  return (
    <div style={{ padding: '8px 0', borderBottom: '1px solid var(--color-border)' }}>
      <div style={{ fontSize: '12px', fontWeight: 400, color: 'var(--color-text-muted)', marginBottom: '4px' }}>
        Cluster {clusterIndex + 1}
      </div>
      {sourceIds.map((id, idx) => {
        const isLast = idx === sourceIds.length - 1
        const prefix = isLast ? '\u2514 ' : '\u251C '
        const memory = memories.get(id)

        let content: string
        if (memory === undefined) {
          // Not yet in the map -- still loading
          content = 'Loading...'
        } else if (memory === null) {
          // Fetch failed -- show raw ID as fallback
          content = id
        } else {
          content = truncate(memory.content, 80)
        }

        const isLoading = memory === undefined
        const isFallback = memory === null

        return (
          <div
            key={id}
            style={{ display: 'flex', gap: '4px', alignItems: 'flex-start' }}
          >
            <span
              style={{
                fontSize: '12px',
                fontWeight: 400,
                color: 'var(--color-text-muted)',
                flexShrink: 0,
                fontFamily: 'ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace',
              }}
            >
              {prefix}
            </span>
            <span
              style={{
                fontSize: isLoading || isFallback ? '12px' : '14px',
                fontWeight: 400,
                color: isLoading ? 'var(--color-text-muted)' : 'var(--color-text)',
              }}
            >
              {content}
            </span>
          </div>
        )
      })}
    </div>
  )
}
