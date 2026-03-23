interface FilterBarProps {
  agents: string[]
  sessions: string[]
  tags: string[]
  selectedAgent: string
  selectedSession: string
  selectedTag: string
  onAgentChange: (value: string) => void
  onSessionChange: (value: string) => void
  onTagChange: (value: string) => void
  onClear: () => void
}

export default function FilterBar(props: FilterBarProps) {
  const {
    agents, sessions, tags,
    selectedAgent, selectedSession, selectedTag,
    onAgentChange, onSessionChange, onTagChange, onClear,
  } = props

  const hasFilters = selectedAgent || selectedSession || selectedTag

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

  const labelStyle = {
    fontSize: '12px',
    fontWeight: 400,
    lineHeight: 1.4,
    color: 'var(--color-text-muted)',
  }

  return (
    <div style={{ display: 'flex', gap: '16px', alignItems: 'center', flexWrap: 'wrap' }}>
      <div style={{ display: 'flex', gap: '4px', alignItems: 'center' }}>
        <span style={labelStyle}>agent:</span>
        <select
          value={selectedAgent}
          onChange={(e) => onAgentChange((e.target as HTMLSelectElement).value)}
          style={selectStyle}
        >
          <option value="">all</option>
          {agents.map(a => (
            <option key={a} value={a}>{a || '(none)'}</option>
          ))}
        </select>
      </div>

      <div style={{ display: 'flex', gap: '4px', alignItems: 'center' }}>
        <span style={labelStyle}>session:</span>
        <select
          value={selectedSession}
          onChange={(e) => onSessionChange((e.target as HTMLSelectElement).value)}
          style={selectStyle}
        >
          <option value="">all</option>
          {sessions.map(s => (
            <option key={s} value={s}>{s || '(none)'}</option>
          ))}
        </select>
      </div>

      <div style={{ display: 'flex', gap: '4px', alignItems: 'center' }}>
        <span style={labelStyle}>tag:</span>
        <select
          value={selectedTag}
          onChange={(e) => onTagChange((e.target as HTMLSelectElement).value)}
          style={selectStyle}
        >
          <option value="">all</option>
          {tags.map(t => (
            <option key={t} value={t}>{t}</option>
          ))}
        </select>
      </div>

      {hasFilters && (
        <button
          onClick={onClear}
          style={{
            padding: '4px 8px',
            fontSize: '12px',
            fontWeight: 400,
            background: 'transparent',
            border: '1px solid var(--color-border)',
            borderRadius: '4px',
            color: 'var(--color-text-muted)',
            cursor: 'pointer',
          }}
        >
          Clear All
        </button>
      )}
    </div>
  )
}
