// @ts-check

const TOKEN_KEY = "op_session_token"

/** @returns {string | null} */
export function getToken() {
  return localStorage.getItem(TOKEN_KEY)
}

/** @param {string} token */
export function setToken(token) {
  localStorage.setItem(TOKEN_KEY, token)
}

export function clearToken() {
  localStorage.removeItem(TOKEN_KEY)
}

/**
 * @param {string} method
 * @param {string} path
 * @param {any=} body
 */
async function request(method, path, body) {
  /** @type {Record<string, string>} */
  const headers = {}
  const token = getToken()
  if (token) headers["Authorization"] = `Bearer ${token}`
  if (body !== undefined) headers["Content-Type"] = "application/json"

  const res = await fetch(path, {
    method,
    headers,
    body: body !== undefined ? JSON.stringify(body) : undefined
  })

  if (res.status === 401) {
    clearToken()
    throw { status: 401, message: "Unauthorized" }
  }

  if (!res.ok) {
    const text = await res.text().catch(() => "")
    throw { status: res.status, message: text || res.statusText }
  }

  if (res.status === 204) return null
  return res.json()
}

/** @param {string} bootstrapToken */
export async function bootstrap(bootstrapToken) {
  const res = await fetch("/api/bootstrap", {
    method: "POST",
    headers: { Authorization: `Bearer ${bootstrapToken}` }
  })
  if (!res.ok) {
    const text = await res.text().catch(() => "")
    throw { status: res.status, message: text || res.statusText }
  }
  return res.json()
}

export async function health() {
  return request("GET", "/health")
}

export async function ready() {
  return request("GET", "/ready")
}

export async function listAgents() {
  return request("GET", "/api/agents")
}

/** @param {string} name */
export async function createAgent(name) {
  return request("POST", "/api/agents", { name })
}

/** @param {string} agentId */
export async function getAgent(agentId) {
  return request("GET", `/api/agents/${encodeURIComponent(agentId)}`)
}

/** @param {string} agentId */
export async function rotateWebhookSecret(agentId) {
  return request(
    "POST",
    `/api/agents/${encodeURIComponent(agentId)}/webhook/rotate`
  )
}

/** @param {string} agentId @param {number} budgetLimitUsd */
export async function setBudget(agentId, budgetLimitUsd) {
  return request("PATCH", `/api/agents/${encodeURIComponent(agentId)}`, {
    budget_limit_usd: budgetLimitUsd
  })
}

/** @param {string} agentId @param {string} content */
export async function sendMessage(agentId, content) {
  return request(
    "POST",
    `/api/agents/${encodeURIComponent(agentId)}/messages`,
    {
      content
    }
  )
}

/**
 * @param {string} agentId
 * @param {{limit?: number, since?: string}=} options
 */
export async function getEvents(agentId, options = {}) {
  const params = new URLSearchParams()
  params.set("limit", String(options.limit ?? 200))
  if (options.since) params.set("since", options.since)
  return request(
    "GET",
    `/api/agents/${encodeURIComponent(agentId)}/events?${params.toString()}`
  )
}
