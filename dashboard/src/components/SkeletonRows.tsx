interface SkeletonRowsProps {
  rows?: number
}

export default function SkeletonRows({ rows = 3 }: SkeletonRowsProps) {
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
      {Array.from({ length: rows }, (_, i) => (
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
  )
}
