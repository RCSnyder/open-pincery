// API client for Open Pincery
const TOKEN_KEY = "op_session_token";

export function getToken() {
  return localStorage.getItem(TOKEN_KEY);
}

export function setToken(token) {
  localStorage.setItem(TOKEN_KEY, token);
}

export function clearToken() {
  localStorage.removeItem(TOKEN_KEY);
}

async function request(method, path, body) {
  const headers = {};
  const token = getToken();
  if (token) headers["Authorization"] = `Bearer ${token}`;
  if (body) headers["Content-Type"] = "application/json";

  const res = await fetch(path, {
    method,
    headers,
    body: body ? JSON.stringify(body) : undefined,
  });

  if (res.status === 401) {
    clearToken();
    // Will be caught by the caller to redirect to auth
    throw { status: 401, message: "Unauthorized" };
  }

  if (!res.ok) {
    const text = await res.text().catch(() => "");
    throw { status: res.status, message: text || res.statusText };
  }

  if (res.status === 204) return null;
  return res.json();
}

export async function bootstrap(bootstrapToken) {
  const res = await fetch("/api/bootstrap", {
    method: "POST",
    headers: { Authorization: `Bearer ${bootstrapToken}` },
  });
  if (!res.ok) {
    const text = await res.text().catch(() => "");
    throw { status: res.status, message: text || res.statusText };
  }
  return res.json();
}

export async function healthCheck() {
  return request("GET", "/health");
}

export async function listAgents() {
  return request("GET", "/api/agents");
}

export async function createAgent(name) {
  return request("POST", "/api/agents", { name });
}

export async function getAgent(id) {
  return request("GET", `/api/agents/${encodeURIComponent(id)}`);
}

export async function sendMessage(agentId, content) {
  return request("POST", `/api/agents/${encodeURIComponent(agentId)}/messages`, { content });
}

export async function getEvents(agentId, limit = 100) {
  return request("GET", `/api/agents/${encodeURIComponent(agentId)}/events?limit=${limit}`);
}
