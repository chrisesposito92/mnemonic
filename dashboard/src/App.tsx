import HealthCard from './components/HealthCard'

export default function App() {
  return (
    <div
      class="min-h-screen flex flex-col items-center justify-center gap-8"
      style={{ background: 'var(--color-bg)', fontFamily: 'ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace' }}
    >
      <h1
        class="text-xl font-semibold"
        style={{ color: 'var(--color-text)', fontSize: '20px', fontWeight: 600, lineHeight: 1.2 }}
      >
        Mnemonic Dashboard
      </h1>
      <HealthCard />
    </div>
  )
}
