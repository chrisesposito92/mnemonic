import { useState } from 'preact/hooks'
import type { Memory } from '../api'

interface MemoryRowProps {
  memory: Memory
}

function relativeTime(iso: string): string {
  const diff = Math.floor((Date.now() - new Date(iso).getTime()) / 1000)
  if (diff < 0) return 'just now'
  if (diff < 60) return `${diff}s ago`
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`
  return `${Math.floor(diff / 86400)}d ago`
}

function truncate(s: string, len: number): string {
  return s.length > len ? s.slice(0, len) + '...' : s
}

export default function MemoryRow({ memory }: MemoryRowProps) {
  const [expanded, setExpanded] = useState(false)

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
    <>
      <tr
        onClick={() => setExpanded(!expanded)}
        style={{
          cursor: 'pointer',
          borderBottom: expanded ? 'none' : '1px solid var(--color-border)',
        }}
      >
        <td style={cellStyle}>{truncate(memory.content, 80)}</td>
        <td style={mutedCellStyle}>{memory.agent_id || '\u2014'}</td>
        <td style={mutedCellStyle}>{memory.session_id || '\u2014'}</td>
        <td style={mutedCellStyle}>{memory.tags.length > 0 ? memory.tags.join(', ') : '\u2014'}</td>
        <td style={mutedCellStyle}>{relativeTime(memory.created_at)}</td>
      </tr>
      {expanded && (
        <tr style={{
          background: 'var(--color-surface)',
          borderTop: '1px solid var(--color-border)',
          borderBottom: '1px solid var(--color-border)',
        }}>
          <td colSpan={5} style={{ padding: '16px 12px' }}>
            <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
              <div style={{ fontSize: '14px', color: 'var(--color-text)', whiteSpace: 'pre-wrap', wordBreak: 'break-word' }}>
                {memory.content}
              </div>
              <div style={{ display: 'flex', flexDirection: 'column', gap: '4px', marginTop: '8px' }}>
                <DetailRow label="id:" value={memory.id} />
                <DetailRow label="embedding_model:" value={memory.embedding_model} />
                <DetailRow label="created_at:" value={memory.created_at} />
                <DetailRow label="updated_at:" value={memory.updated_at ?? '\u2014'} />
              </div>
            </div>
          </td>
        </tr>
      )}
    </>
  )
}

function DetailRow({ label, value }: { label: string; value: string }) {
  return (
    <div style={{ display: 'flex', gap: '8px', alignItems: 'baseline' }}>
      <span style={{ fontSize: '12px', fontWeight: 400, lineHeight: 1.4, color: 'var(--color-text-muted)' }}>
        {label}
      </span>
      <span style={{ fontSize: '14px', fontWeight: 400, lineHeight: 1.5, color: 'var(--color-text)' }}>
        {value}
      </span>
    </div>
  )
}
