// dashboard/src/api.ts -- Typed API client for mnemonic dashboard
// All fetch calls use this module for consistent auth, timeout, abort, and 401 handling.

const API_TIMEOUT_MS = 10_000

// -- Types -----------------------------------------------------------------------

export interface HealthResponse {
  status: string
  backend: string
  auth_enabled: boolean
}

export interface Memory {
  id: string
  content: string
  agent_id: string
  session_id: string
  tags: string[]
  embedding_model: string
  created_at: string
  updated_at: string | null
}

export interface ListResponse {
  memories: Memory[]
  total: number
}

export interface SearchResultItem extends Memory {
  distance: number
}

export interface SearchResponse {
  memories: SearchResultItem[]
}

export interface AgentStats {
  agent_id: string
  memory_count: number
  last_active: string
}

export interface StatsResponse {
  agents: AgentStats[]
}

// -- Error types -----------------------------------------------------------------

export class ApiError extends Error {
  constructor(public status: number, message: string) {
    super(message)
  }
}

export class UnauthorizedError extends ApiError {
  constructor(message: string = 'unauthorized') {
    super(401, message)
  }
}

// -- Core fetch wrapper ----------------------------------------------------------

/**
 * Fetch wrapper with auth header, timeout, and abort support.
 * @param url - API endpoint path (e.g., '/health')
 * @param token - Bearer token (null in open mode)
 * @param signal - Optional AbortSignal for request cancellation (review concern #6)
 * @param init - Additional RequestInit options
 *
 * Throws UnauthorizedError on 401/403 -- callers (App.tsx) catch this
 * to clear token and return to login gate (review concern #8).
 */
export async function apiFetch(
  url: string,
  token: string | null,
  signal?: AbortSignal | null,
  init?: RequestInit,
): Promise<Response> {
  const headers: Record<string, string> = {
    ...(init?.headers as Record<string, string> ?? {}),
  }
  if (token) {
    headers['Authorization'] = `Bearer ${token}`
  }

  // Combine caller's abort signal with a timeout signal
  const timeoutSignal = AbortSignal.timeout(API_TIMEOUT_MS)
  const effectiveSignal = signal
    ? AbortSignal.any([signal, timeoutSignal])
    : timeoutSignal

  const response = await fetch(url, {
    ...init,
    headers,
    signal: effectiveSignal,
  })

  if (response.status === 401 || response.status === 403) {
    throw new UnauthorizedError(
      response.status === 401 ? 'unauthorized' : 'forbidden'
    )
  }

  if (!response.ok) {
    throw new ApiError(response.status, `HTTP ${response.status}`)
  }

  return response
}

// -- Typed endpoint wrappers -----------------------------------------------------

/** GET /health -- always public, never needs auth token */
export async function fetchHealth(signal?: AbortSignal | null): Promise<HealthResponse> {
  // Health is public -- never send a token (even if user has one)
  const resp = await fetch('/health', {
    signal: signal ?? AbortSignal.timeout(API_TIMEOUT_MS),
  })
  if (!resp.ok) throw new ApiError(resp.status, `HTTP ${resp.status}`)
  return resp.json()
}

/** GET /memories with query params */
export async function fetchMemories(
  token: string | null,
  params: {
    limit?: number
    offset?: number
    agent_id?: string
    session_id?: string
    tag?: string
  },
  signal?: AbortSignal | null,
): Promise<ListResponse> {
  const qs = new URLSearchParams()
  if (params.limit != null) qs.set('limit', String(params.limit))
  if (params.offset != null) qs.set('offset', String(params.offset))
  if (params.agent_id) qs.set('agent_id', params.agent_id)
  if (params.session_id) qs.set('session_id', params.session_id)
  if (params.tag) qs.set('tag', params.tag)

  const url = `/memories${qs.toString() ? '?' + qs.toString() : ''}`
  const resp = await apiFetch(url, token, signal)
  return resp.json()
}

/** GET /stats -- returns per-agent breakdown */
export async function fetchStats(
  token: string | null,
  signal?: AbortSignal | null,
): Promise<StatsResponse> {
  const resp = await apiFetch('/stats', token, signal)
  return resp.json()
}

/** GET /memories/search with query params */
export async function searchMemories(
  token: string | null,
  params: {
    q: string
    agent_id?: string
    tag?: string
    limit?: number
  },
  signal?: AbortSignal | null,
): Promise<SearchResponse> {
  const qs = new URLSearchParams()
  qs.set('q', params.q)
  if (params.agent_id) qs.set('agent_id', params.agent_id)
  if (params.tag) qs.set('tag', params.tag)
  if (params.limit != null) qs.set('limit', String(params.limit))

  const resp = await apiFetch(`/memories/search?${qs.toString()}`, token, signal)
  return resp.json()
}
