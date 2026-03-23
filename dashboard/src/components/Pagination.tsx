interface PaginationProps {
  offset: number
  limit: number
  total: number
  onPrev: () => void
  onNext: () => void
}

export default function Pagination({ offset, limit, total, onPrev, onNext }: PaginationProps) {
  const start = total === 0 ? 0 : offset + 1
  const end = Math.min(offset + limit, total)
  const hasPrev = offset > 0
  const hasNext = offset + limit < total

  const buttonStyle = (enabled: boolean) => ({
    padding: '6px 12px',
    fontSize: '12px',
    fontWeight: 400,
    fontFamily: 'inherit',
    background: enabled ? 'var(--color-surface)' : 'transparent',
    border: `1px solid ${enabled ? 'var(--color-border)' : 'transparent'}`,
    borderRadius: '4px',
    color: enabled ? 'var(--color-text)' : 'var(--color-text-muted)',
    cursor: enabled ? 'pointer' : 'default',
    opacity: enabled ? 1 : 0.4,
  })

  return (
    <div style={{
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'space-between',
      paddingTop: '16px',
    }}>
      <span style={{
        fontSize: '12px',
        fontWeight: 400,
        lineHeight: 1.4,
        color: 'var(--color-text-muted)',
      }}>
        {total > 0 ? `Showing ${start}\u2013${end} of ${total}` : ''}
      </span>
      <div style={{ display: 'flex', gap: '8px' }}>
        <button
          onClick={onPrev}
          disabled={!hasPrev}
          style={buttonStyle(hasPrev)}
        >
          Prev
        </button>
        <button
          onClick={onNext}
          disabled={!hasNext}
          style={buttonStyle(hasNext)}
        >
          Next
        </button>
      </div>
    </div>
  )
}
