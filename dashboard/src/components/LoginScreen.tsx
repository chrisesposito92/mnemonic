import { useState } from 'preact/hooks'
import { apiFetch, UnauthorizedError } from '../api'

interface LoginScreenProps {
  onLogin: (token: string) => void
  error?: string
}

export default function LoginScreen({ onLogin, error: initialError }: LoginScreenProps) {
  const [value, setValue] = useState('')
  const [submitting, setSubmitting] = useState(false)
  const [error, setError] = useState(initialError ?? '')

  const handleSubmit = async (e: Event) => {
    e.preventDefault()
    const token = value.trim()
    if (!token) return

    setSubmitting(true)
    setError('')

    try {
      await apiFetch('/memories?limit=1', token)
      onLogin(token)
    } catch (err) {
      if (err instanceof UnauthorizedError) {
        setError('Invalid API key. Check your key and try again.')
      } else {
        setError('Could not reach API. Check that the server is running and reload the page.')
      }
      setValue('')
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <div style={{
      display: 'flex',
      flexDirection: 'column',
      alignItems: 'center',
      justifyContent: 'center',
      minHeight: '100vh',
      background: 'var(--color-bg)',
      gap: '16px',
      padding: '24px',
    }}>
      <h1 style={{
        fontSize: '20px', fontWeight: 600, lineHeight: 1.2,
        color: 'var(--color-text)',
      }}>
        Mnemonic
      </h1>
      <span style={{
        fontSize: '14px', fontWeight: 400,
        color: 'var(--color-text-muted)',
      }}>
        API key required
      </span>
      <form onSubmit={handleSubmit} style={{
        display: 'flex', flexDirection: 'column', gap: '12px',
        width: '100%', maxWidth: '400px',
      }}>
        <input
          type="password"
          placeholder="mnk_..."
          value={value}
          onInput={(e) => setValue((e.target as HTMLInputElement).value)}
          disabled={submitting}
          autoComplete="off"
          style={{
            width: '100%', padding: '10px 12px',
            fontSize: '14px', fontWeight: 400, fontFamily: 'inherit',
            background: 'var(--color-surface)',
            border: '1px solid var(--color-border)', borderRadius: '4px',
            color: 'var(--color-text)', outline: 'none',
          }}
        />
        <button
          type="submit"
          disabled={submitting || !value.trim()}
          style={{
            padding: '10px 16px', fontSize: '14px', fontWeight: 600,
            background: 'var(--color-accent)', color: 'var(--color-bg)',
            border: 'none', borderRadius: '4px',
            cursor: submitting ? 'wait' : 'pointer',
            opacity: submitting || !value.trim() ? 0.6 : 1,
          }}
        >
          {submitting ? 'Connecting...' : 'Connect'}
        </button>
      </form>
      {error && (
        <div style={{
          fontSize: '14px', fontWeight: 400,
          color: 'var(--color-error)',
          maxWidth: '400px', textAlign: 'center',
        }}>
          {error}
        </div>
      )}
    </div>
  )
}
