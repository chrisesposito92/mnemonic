import { useState, useEffect, useCallback } from 'preact/hooks'
import { fetchHealth } from './api'
import Header from './components/Header'
import TabBar, { type Tab } from './components/TabBar'
import LoginScreen from './components/LoginScreen'
import SkeletonRows from './components/SkeletonRows'
import MemoriesTab from './components/MemoriesTab'

// -- Hash Router (D-04) ----------------------------------------------------------

function parseHash(): Tab {
  const hash = window.location.hash
  if (hash === '#/agents') return 'agents'
  if (hash === '#/search') return 'search'
  return 'memories' // default
}

// -- App State Machine -----------------------------------------------------------

type AppState =
  | { kind: 'checking' }                        // Initial mount, health probe in flight
  | { kind: 'login'; error?: string }           // Auth enabled, show login screen
  | { kind: 'dashboard'; token: string | null } // Authenticated or open mode

// -- App Component ---------------------------------------------------------------

export default function App() {
  const [appState, setAppState] = useState<AppState>({ kind: 'checking' })
  const [activeTab, setActiveTab] = useState<Tab>(parseHash())

  // Hash router listener (D-04)
  useEffect(() => {
    const handler = () => setActiveTab(parseHash())
    window.addEventListener('hashchange', handler)
    return () => window.removeEventListener('hashchange', handler)
  }, [])

  // Auth detection via GET /health auth_enabled field (review concern #2)
  useEffect(() => {
    const controller = new AbortController()

    fetchHealth(controller.signal)
      .then(data => {
        if (data.auth_enabled) {
          setAppState({ kind: 'login' })
        } else {
          setAppState({ kind: 'dashboard', token: null })
        }
      })
      .catch(() => {
        setAppState({
          kind: 'login',
          error: 'Could not reach API. Check that the server is running and reload the page.'
        })
      })

    return () => controller.abort()
  }, [])

  // Handle successful login
  const handleLogin = useCallback((token: string) => {
    setAppState({ kind: 'dashboard', token })
  }, [])

  // Handle unauthorized response from any protected endpoint (review concern #8)
  const handleUnauthorized = useCallback(() => {
    setAppState({ kind: 'login', error: 'Session expired. Please reconnect.' })
  }, [])

  // -- Render --------------------------------------------------------------------

  if (appState.kind === 'checking') {
    return (
      <div style={{
        minHeight: '100vh',
        background: 'var(--color-bg)',
        fontFamily: 'ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        padding: '24px',
      }}>
        <SkeletonRows rows={3} />
      </div>
    )
  }

  if (appState.kind === 'login') {
    return (
      <div style={{
        fontFamily: 'ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace',
      }}>
        <LoginScreen onLogin={handleLogin} error={appState.error} />
      </div>
    )
  }

  // Dashboard mode
  const { token } = appState

  return (
    <div style={{
      minHeight: '100vh',
      background: 'var(--color-bg)',
      fontFamily: 'ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace',
      display: 'flex',
      flexDirection: 'column',
    }}>
      <Header token={token} />
      <TabBar activeTab={activeTab} />
      <div style={{ padding: '32px 24px', flex: 1 }}>
        {activeTab === 'memories' && (
          <MemoriesTab token={token} onUnauthorized={handleUnauthorized} />
        )}
        {activeTab === 'agents' && (
          <div style={{ color: 'var(--color-text-muted)', fontSize: '14px' }}>
            Agents tab -- implemented in Plan 04
          </div>
        )}
        {activeTab === 'search' && (
          <div style={{ color: 'var(--color-text-muted)', fontSize: '14px' }}>
            Search tab -- implemented in Plan 04
          </div>
        )}
      </div>
    </div>
  )
}
