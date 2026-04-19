// Open Pincery — Single-Page Application
import * as api from "./api.js";

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------
let state = {
  view: "loading", // loading | auth | dashboard | agent
  agents: [],
  selectedAgentId: null,
  selectedAgent: null,
  events: [],
  pollTimer: null,
  health: null,
};

const app = document.getElementById("app");

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------
function navigate(view, opts = {}) {
  if (state.pollTimer) {
    clearInterval(state.pollTimer);
    state.pollTimer = null;
  }
  Object.assign(state, opts);
  state.view = view;
  render();
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------
function h(tag, attrs, ...children) {
  const el = document.createElement(tag);
  if (attrs) {
    for (const [k, v] of Object.entries(attrs)) {
      if (k === "class") el.className = v;
      else if (k.startsWith("on")) el.addEventListener(k.slice(2).toLowerCase(), v);
      else if (k === "htmlFor") el.htmlFor = v;
      else el.setAttribute(k, v);
    }
  }
  for (const c of children.flat()) {
    if (c == null || c === false) continue;
    el.append(typeof c === "string" || typeof c === "number" ? document.createTextNode(String(c)) : c);
  }
  return el;
}

function timeAgo(iso) {
  const diff = Date.now() - new Date(iso).getTime();
  const s = Math.floor(diff / 1000);
  if (s < 60) return `${s}s ago`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m}m ago`;
  const hrs = Math.floor(m / 60);
  if (hrs < 24) return `${hrs}h ago`;
  return `${Math.floor(hrs / 24)}d ago`;
}

function statusClass(status) {
  if (status === "awake" || status === "wake_acquiring") return "is-awake";
  if (status === "maintenance" || status === "wake_ending") return "is-maintenance";
  return "is-asleep";
}

function statusLabel(status) {
  return status.replace(/_/g, " ");
}

function eventTypeIcon(type) {
  switch (type) {
    case "message_received": return "💬";
    case "message_sent": return "📤";
    case "tool_call": return "🔧";
    case "tool_result": return "📋";
    case "wake_start": return "⏰";
    case "wake_end": return "😴";
    case "plan": return "📝";
    default: return "📌";
  }
}

function toast(message, isError = false) {
  const existing = document.querySelector(".toast");
  if (existing) existing.remove();
  const el = h("div", { class: "toast" },
    h("span", { style: isError ? "color: var(--danger)" : "color: var(--accent)" },
      isError ? "✗ " : "✓ "
    ),
    message
  );
  document.body.appendChild(el);
  setTimeout(() => el.remove(), 4000);
}

// ---------------------------------------------------------------------------
// Views
// ---------------------------------------------------------------------------

function renderAuth() {
  const form = h("form", { class: "form", onSubmit: handleBootstrap },
    h("label", { class: "label" },
      "Bootstrap Token",
      h("input", {
        class: "input",
        type: "password",
        id: "bootstrap-token",
        placeholder: "Paste your BOOTSTRAP_TOKEN here",
        required: "required",
        autocomplete: "off",
      })
    ),
    h("button", { class: "button", type: "submit" }, "Bootstrap System"),
    h("div", { class: "muted", style: "font-size: 13px; margin-top: 8px" },
      "If the system is already bootstrapped, paste your session token below instead."
    ),
    h("label", { class: "label", style: "margin-top: 12px" },
      "Or Enter Session Token",
      h("input", {
        class: "input",
        type: "password",
        id: "session-token",
        placeholder: "Paste an existing session token",
        autocomplete: "off",
      })
    ),
    h("button", { class: "button-secondary", type: "button", onClick: handleSessionLogin },
      "Login with Session Token"
    )
  );

  return h("div", { class: "auth-layout" },
    h("div", { class: "auth-card" },
      h("div", { class: "auth-panel" },
        h("h1", { class: "auth-title" }, "Open\nPincery"),
        h("p", { class: "auth-copy" },
          "A self-hosted autonomous agent runtime. Your agents wake, think, act, and maintain themselves — all under your control."
        ),
        h("div", { class: "auth-points" },
          h("div", { class: "auth-point" },
            h("strong", null, "CAS Lifecycle"),
            h("span", { class: "muted" }, "Compare-and-swap state transitions prevent race conditions")
          ),
          h("div", { class: "auth-point" },
            h("strong", null, "Event Sourced"),
            h("span", { class: "muted" }, "Every action is an immutable, auditable event")
          ),
          h("div", { class: "auth-point" },
            h("strong", null, "Self-Maintaining"),
            h("span", { class: "muted" }, "Agents build identity and work lists through maintenance cycles")
          )
        )
      ),
      h("div", { class: "auth-form-panel" },
        h("h2", { style: "margin: 0 0 8px; font-family: var(--font-display)" }, "Get Started"),
        h("p", { class: "card-copy", style: "margin-bottom: 20px" },
          "Bootstrap a fresh system or login with an existing session token."
        ),
        form
      )
    )
  );
}

async function handleBootstrap(e) {
  e.preventDefault();
  const tokenInput = document.getElementById("bootstrap-token");
  const token = tokenInput.value.trim();
  if (!token) return;

  try {
    const result = await api.bootstrap(token);
    api.setToken(result.session_token);
    toast("System bootstrapped! Logged in as admin.");
    await loadDashboard();
  } catch (err) {
    if (err.status === 409) {
      toast("System already bootstrapped. Use a session token to login.", true);
    } else {
      toast(`Bootstrap failed: ${err.message}`, true);
    }
  }
}

async function handleSessionLogin() {
  const tokenInput = document.getElementById("session-token");
  const token = tokenInput.value.trim();
  if (!token) {
    toast("Please enter a session token.", true);
    return;
  }
  api.setToken(token);
  try {
    await api.listAgents();
    toast("Logged in successfully.");
    await loadDashboard();
  } catch {
    api.clearToken();
    toast("Invalid session token.", true);
  }
}

function renderDashboard() {
  const sidebar = h("nav", { class: "sidebar" },
    h("div", { class: "brand" },
      h("span", { class: "brand-kicker" }, "Agent Runtime"),
      h("h1", { class: "brand-title" }, "Open Pincery"),
      h("p", { class: "brand-copy" }, "Autonomous agent runtime with CAS lifecycle, event sourcing, and self-maintenance.")
    ),
    h("div", { class: "sidebar-section" },
      h("span", { class: "sidebar-label" }, "Agents"),
      h("div", { class: "agent-list" },
        ...state.agents.map(a =>
          h("div", {
            class: `agent-list-item${state.selectedAgentId === a.id ? " is-active" : ""}`,
            onClick: () => selectAgent(a.id),
          },
            h("span", { class: "agent-list-name" }, a.name),
            h("div", { class: "agent-list-meta" },
              h("span", { class: `status-dot ${statusClass(a.status)}` }),
              h("span", null, statusLabel(a.status)),
              h("span", null, "·"),
              h("span", null, timeAgo(a.created_at))
            )
          )
        ),
        state.agents.length === 0
          ? h("div", { class: "empty-state", style: "padding: 16px" }, "No agents yet")
          : null
      )
    ),
    h("div", { class: "sidebar-actions" },
      h("button", { class: "button", onClick: showCreateAgent }, "+ New Agent"),
      h("button", { class: "button-danger", onClick: handleLogout }, "Logout")
    )
  );

  let content;
  if (state.selectedAgent) {
    content = renderAgentDetail();
  } else {
    content = renderOverview();
  }

  return h("div", { class: "shell" }, sidebar, content);
}

function renderOverview() {
  const total = state.agents.length;
  const awake = state.agents.filter(a => a.status === "awake" || a.status === "wake_acquiring").length;
  const resting = state.agents.filter(a => a.status === "resting" || a.status === "asleep").length;
  const maintenance = state.agents.filter(a => a.status === "maintenance" || a.status === "wake_ending").length;

  return h("main", { class: "content" },
    h("div", { class: "hero" },
      h("h2", { class: "hero-title" }, "Agent Dashboard"),
      h("p", { class: "hero-copy" },
        "Monitor your autonomous agents, send messages, and inspect event logs in real time."
      )
    ),
    h("div", { class: "grid" },
      h("div", { class: "card span-4" },
        h("div", { class: "metric" },
          h("span", { class: "metric-label" }, "Total Agents"),
          h("span", { class: "metric-value" }, String(total)),
        )
      ),
      h("div", { class: "card span-4" },
        h("div", { class: "metric" },
          h("span", { class: "metric-label" }, "Awake"),
          h("span", { class: "metric-value", style: "color: var(--accent-2)" }, String(awake)),
        )
      ),
      h("div", { class: "card span-4" },
        h("div", { class: "metric" },
          h("span", { class: "metric-label" }, "Resting"),
          h("span", { class: "metric-value" }, String(resting)),
          maintenance > 0
            ? h("span", { class: "metric-subtle" }, `${maintenance} in maintenance`)
            : null
        )
      ),
      h("div", { class: "card span-12" },
        h("h3", null, "System Health"),
        state.health
          ? h("div", { class: "kv-list" },
              h("div", { class: "kv-row" },
                h("span", { class: "kv-key" }, "Status"),
                h("span", { class: "kv-value" },
                  h("span", { class: `pill` },
                    h("span", { class: `status-dot ${state.health.status === "ok" ? "is-awake" : "is-maintenance"}` }),
                    state.health.status
                  )
                )
              ),
              h("div", { class: "kv-row" },
                h("span", { class: "kv-key" }, "Database"),
                h("span", { class: "kv-value" }, state.health.db)
              )
            )
          : h("span", { class: "muted" }, "Checking...")
      )
    )
  );
}

function renderAgentDetail() {
  const a = state.selectedAgent;
  if (!a) return h("main", { class: "content" }, h("div", { class: "empty-state" }, "Loading..."));

  return h("main", { class: "content" },
    h("div", { style: "display: flex; justify-content: space-between; align-items: flex-start" },
      h("div", null,
        h("h2", { style: "font-family: var(--font-display); margin: 0 0 4px; letter-spacing: -0.03em" }, a.name),
        h("div", { style: "display: flex; gap: 8px; align-items: center" },
          h("span", { class: `status-dot ${statusClass(a.status)}` }),
          h("span", { class: "muted" }, statusLabel(a.status))
        )
      ),
      h("button", { class: "button-secondary", onClick: () => { state.selectedAgentId = null; state.selectedAgent = null; state.events = []; render(); } }, "← Back")
    ),

    // Agent info
    h("div", { class: "grid" },
      h("div", { class: "card span-6" },
        h("h3", null, "Details"),
        h("div", { class: "kv-list" },
          h("div", { class: "kv-row" },
            h("span", { class: "kv-key" }, "ID"),
            h("span", { class: "kv-value" }, h("code", null, a.id))
          ),
          h("div", { class: "kv-row" },
            h("span", { class: "kv-key" }, "Created"),
            h("span", { class: "kv-value" }, new Date(a.created_at).toLocaleString())
          ),
          a.identity ? h("div", { class: "kv-row" },
            h("span", { class: "kv-key" }, "Identity"),
            h("span", { class: "kv-value" }, a.identity)
          ) : null,
          a.work_list ? h("div", { class: "kv-row" },
            h("span", { class: "kv-key" }, "Work List"),
            h("span", { class: "kv-value" }, a.work_list)
          ) : null
        )
      ),

      // Send message
      h("div", { class: "card span-6" },
        h("h3", null, "Send Message"),
        h("form", { class: "form", onSubmit: handleSendMessage },
          h("textarea", {
            class: "textarea",
            id: "message-input",
            placeholder: "Type a message for this agent...",
            style: "min-height: 100px",
          }),
          h("button", { class: "button", type: "submit" }, "Send Message")
        )
      )
    ),

    // Event log
    h("div", { class: "card span-12", style: "grid-column: span 12" },
      h("div", { style: "display: flex; justify-content: space-between; align-items: center; margin-bottom: 12px" },
        h("h3", { style: "margin: 0" }, "Event Log"),
        h("button", { class: "button-secondary", style: "padding: 8px 12px; font-size: 12px", onClick: () => loadEvents(a.id) }, "↻ Refresh")
      ),
      state.events.length > 0
        ? h("div", { class: "feed" },
            ...state.events.map(ev =>
              h("div", { class: "event" },
                h("div", { class: "event-header" },
                  h("span", null,
                    h("span", { class: "event-type" }, `${eventTypeIcon(ev.event_type)} ${ev.event_type}`),
                    h("span", { class: "muted" }, ` · ${ev.source}`)
                  ),
                  h("span", null, timeAgo(ev.created_at))
                ),
                ev.content
                  ? h("div", { class: "event-content" }, ev.content)
                  : null,
                ev.tool_name
                  ? h("div", { class: "event-content", style: "margin-top: 8px" },
                      h("code", null, ev.tool_name),
                      ev.tool_input
                        ? h("pre", { style: "margin: 6px 0 0; font-size: 12px; color: var(--text-muted); white-space: pre-wrap; word-break: break-word" }, ev.tool_input)
                        : null,
                      ev.tool_output
                        ? h("pre", { style: "margin: 6px 0 0; font-size: 12px; color: var(--accent); white-space: pre-wrap; word-break: break-word" }, ev.tool_output)
                        : null
                    )
                  : null,
                ev.termination_reason
                  ? h("div", { style: "margin-top: 6px; color: var(--warning); font-size: 12px" },
                      `Termination: ${ev.termination_reason}`
                    )
                  : null
              )
            )
          )
        : h("div", { class: "empty-state" }, "No events recorded yet. Send a message to get started.")
    )
  );
}

// ---------------------------------------------------------------------------
// Actions
// ---------------------------------------------------------------------------

async function loadDashboard() {
  try {
    const [agents, health] = await Promise.all([
      api.listAgents(),
      api.healthCheck().catch(() => null),
    ]);
    navigate("dashboard", { agents, health, selectedAgentId: null, selectedAgent: null, events: [] });
  } catch (err) {
    if (err.status === 401) {
      navigate("auth");
    } else {
      toast(`Failed to load: ${err.message}`, true);
    }
  }
}

async function selectAgent(id) {
  state.selectedAgentId = id;
  render();
  try {
    const [agent, eventsResp] = await Promise.all([
      api.getAgent(id),
      api.getEvents(id, 200),
    ]);
    state.selectedAgent = agent;
    state.events = eventsResp.events || [];
    render();

    // Start polling events every 5 seconds
    if (state.pollTimer) clearInterval(state.pollTimer);
    state.pollTimer = setInterval(() => loadEvents(id), 5000);
  } catch (err) {
    toast(`Failed to load agent: ${err.message}`, true);
  }
}

async function loadEvents(agentId) {
  try {
    const eventsResp = await api.getEvents(agentId, 200);
    state.events = eventsResp.events || [];

    // Also refresh agent status
    const agent = await api.getAgent(agentId);
    state.selectedAgent = agent;

    // Refresh agent list
    const agents = await api.listAgents();
    state.agents = agents;

    render();
  } catch {
    // Silently fail on polls
  }
}

function showCreateAgent() {
  const name = prompt("Agent name:");
  if (!name || !name.trim()) return;
  createNewAgent(name.trim());
}

async function createNewAgent(name) {
  try {
    await api.createAgent(name);
    toast(`Agent "${name}" created.`);
    const agents = await api.listAgents();
    state.agents = agents;
    render();
  } catch (err) {
    toast(`Failed to create agent: ${err.message}`, true);
  }
}

async function handleSendMessage(e) {
  e.preventDefault();
  const input = document.getElementById("message-input");
  const content = input.value.trim();
  if (!content || !state.selectedAgentId) return;

  try {
    await api.sendMessage(state.selectedAgentId, content);
    input.value = "";
    toast("Message sent.");
    // Refresh events after a short delay to let the agent process
    setTimeout(() => loadEvents(state.selectedAgentId), 1000);
  } catch (err) {
    toast(`Failed to send message: ${err.message}`, true);
  }
}

function handleLogout() {
  api.clearToken();
  navigate("auth");
  toast("Logged out.");
}

// ---------------------------------------------------------------------------
// Render
// ---------------------------------------------------------------------------
function render() {
  let content;
  switch (state.view) {
    case "auth":
      content = renderAuth();
      break;
    case "dashboard":
      content = renderDashboard();
      break;
    default:
      content = h("div", { class: "auth-layout" },
        h("div", { style: "text-align: center" },
          h("h1", { class: "brand-title" }, "Open Pincery"),
          h("p", { class: "muted" }, "Loading...")
        )
      );
  }
  app.replaceChildren(content);
}

// ---------------------------------------------------------------------------
// Init
// ---------------------------------------------------------------------------
async function init() {
  render(); // Show loading
  if (api.getToken()) {
    try {
      await loadDashboard();
    } catch {
      navigate("auth");
    }
  } else {
    navigate("auth");
  }
}

init();
