interface DistanceBarProps {
  distance: number
}

export default function DistanceBar({ distance }: DistanceBarProps) {
  // distance 0.0 = identical -> 100% fill
  // distance 1.0 = dissimilar -> 0% fill
  // Clamped to 0-100% for backends that return distances outside 0-1 range
  // (review concern #5: SQLite KNN may return raw L2 distances > 1.0)
  const fillPercent = Math.min(100, Math.max(0, (1 - distance) * 100))

  return (
    <div style={{ display: 'flex', alignItems: 'center', gap: '8px', minWidth: '160px' }}>
      <div style={{
        flex: 1,
        height: '8px',
        background: 'rgba(34, 211, 238, 0.3)',
        borderRadius: '4px',
        overflow: 'hidden',
      }}>
        <div style={{
          width: `${fillPercent}%`,
          height: '100%',
          background: 'var(--color-accent)',
          borderRadius: '4px',
        }} />
      </div>
      <span style={{
        fontSize: '14px',
        fontWeight: 400,
        color: 'var(--color-text)',
        fontVariantNumeric: 'tabular-nums',
        minWidth: '50px',
        textAlign: 'right',
      }}>
        {distance.toFixed(4)}
      </span>
    </div>
  )
}
