export type Tab = 'memories' | 'agents' | 'search'

interface TabBarProps {
  activeTab: Tab
}

const TABS: { id: Tab; label: string; href: string }[] = [
  { id: 'memories', label: 'Memories', href: '#/memories' },
  { id: 'agents', label: 'Agents', href: '#/agents' },
  { id: 'search', label: 'Search', href: '#/search' },
]

export default function TabBar({ activeTab }: TabBarProps) {
  return (
    <div style={{
      display: 'flex',
      gap: '0px',
      borderBottom: '1px solid var(--color-border)',
      paddingLeft: '24px',
    }}>
      {TABS.map(tab => (
        <a
          key={tab.id}
          href={tab.href}
          style={{
            padding: '12px 16px',
            fontSize: '14px',
            fontWeight: 400,
            lineHeight: 1.5,
            color: tab.id === activeTab ? 'var(--color-text)' : 'var(--color-text-muted)',
            textDecoration: 'none',
            borderBottom: tab.id === activeTab
              ? '2px solid var(--color-accent)'
              : '2px solid transparent',
            marginBottom: '-1px',
          }}
        >
          {tab.label}
        </a>
      ))}
    </div>
  )
}
