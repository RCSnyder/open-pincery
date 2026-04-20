// @ts-check
import * as api from "./api.js"

/** @typedef {{id:string,name:string,status:string,created_at:string,budget_limit_usd?:number,budget_used_usd?:number}} Agent */
/** @typedef {{id:string,event_type:string,source:string,content?:string,tool_name?:string,tool_output?:string,termination_reason?:string,created_at:string}} EventRow */

/** @type {{agents: Agent[], selected: Agent|null, events: EventRow[], pollSince: string|null, pollDelayMs: number, pollTimer: number|null, rotatingSecret: string|null}} */
const state = {
  agents: [],
  selected: null,
  events: [],
  pollSince: null,
  pollDelayMs: 4000,
  pollTimer: null,
  rotatingSecret: null
}

const app = document.getElementById("app")

function stopPoll() {
  if (state.pollTimer !== null) {
    window.clearTimeout(state.pollTimer)
    state.pollTimer = null
  }
}

/** @param {string} msg @param {boolean=} isError */
function toast(msg, isError = false) {
  const old = document.getElementById("toast")
  if (old) old.remove()
  const el = document.createElement("div")
  el.id = "toast"
  el.className = `toast ${isError ? "error" : "ok"}`
  el.textContent = msg
  document.body.appendChild(el)
  window.setTimeout(() => el.remove(), 3000)
}

/** @param {string} hash */
function routeTo(hash) {
  if (window.location.hash !== hash) {
    window.location.hash = hash
    return
  }
  render()
}

function currentRoute() {
  const hash = window.location.hash || "#/login"
  const settingsMatch = hash.match(/^#\/agents\/([^/]+)\/settings$/)
  if (settingsMatch) {
    return { view: "settings", agentId: decodeURIComponent(settingsMatch[1]) }
  }
  const detailMatch = hash.match(/^#\/agents\/([^/]+)$/)
  if (detailMatch) {
    return { view: "detail", agentId: decodeURIComponent(detailMatch[1]) }
  }
  if (hash === "#/agents") return { view: "agents" }
  return { view: "login" }
}

/** @param {string} iso */
function age(iso) {
  const sec = Math.max(
    0,
    Math.floor((Date.now() - new Date(iso).getTime()) / 1000)
  )
  if (sec < 60) return `${sec}s`
  if (sec < 3600) return `${Math.floor(sec / 60)}m`
  if (sec < 86400) return `${Math.floor(sec / 3600)}h`
  return `${Math.floor(sec / 86400)}d`
}

/** @returns {Promise<void>} */
async function loadAgents() {
  state.agents = await api.listAgents()
}

/** @param {string} agentId */
async function loadDetail(agentId) {
  state.selected = await api.getAgent(agentId)
  const first = await api.getEvents(agentId, { limit: 200 })
  state.events = Array.isArray(first.events) ? first.events : []
  state.pollSince =
    state.events.length > 0 ? state.events[state.events.length - 1].id : null
  state.pollDelayMs = 4000
}

/** @param {string} agentId */
async function pollEvents(agentId) {
  stopPoll()
  try {
    const res = await api.getEvents(agentId, {
      since: state.pollSince || undefined,
      limit: 200
    })
    const incoming = Array.isArray(res.events) ? res.events : []
    if (incoming.length > 0) {
      state.events = state.events.concat(incoming)
      state.pollSince = incoming[incoming.length - 1].id
      state.selected = await api.getAgent(agentId)
      render()
    }
    state.pollDelayMs = 4000
  } catch (_e) {
    state.pollDelayMs = Math.min(state.pollDelayMs * 2, 32000)
  }
  state.pollTimer = window.setTimeout(() => {
    pollEvents(agentId).catch(() => {})
  }, state.pollDelayMs)
}

function nav() {
  const wrap = document.createElement("nav")
  wrap.className = "topnav"
  wrap.innerHTML = `
    <a href="#/agents">Agents</a>
    <a href="#/login">Login</a>
    <button id="logout-btn" type="button">Logout</button>
  `
  wrap.querySelector("#logout-btn")?.addEventListener("click", () => {
    api.clearToken()
    stopPoll()
    routeTo("#/login")
  })
  return wrap
}

function renderLogin() {
  stopPoll()
  const root = document.createElement("div")
  root.className = "page"
  root.innerHTML = `
    <h1>Open Pincery</h1>
    <p class="muted">Paste a session token or bootstrap a fresh system.</p>
    <form id="bootstrap-form" class="card stack">
      <label>Bootstrap Token<input id="bootstrap-token" type="password" required /></label>
      <button type="submit">Bootstrap</button>
    </form>
    <form id="session-form" class="card stack">
      <label>Session Token<input id="session-token" type="password" required /></label>
      <button type="submit">Login</button>
    </form>
  `

  root.querySelector("#bootstrap-form")?.addEventListener("submit", async e => {
    e.preventDefault()
    const token = /** @type {HTMLInputElement} */ (
      root.querySelector("#bootstrap-token")
    ).value.trim()
    try {
      const out = await api.bootstrap(token)
      api.setToken(out.session_token)
      toast("Bootstrapped and logged in.")
      await loadAgents()
      routeTo("#/agents")
    } catch (err) {
      toast(`Bootstrap failed: ${err.message || err}`, true)
    }
  })

  root.querySelector("#session-form")?.addEventListener("submit", async e => {
    e.preventDefault()
    const token = /** @type {HTMLInputElement} */ (
      root.querySelector("#session-token")
    ).value.trim()
    try {
      api.setToken(token)
      await loadAgents()
      toast("Logged in.")
      routeTo("#/agents")
    } catch (_err) {
      api.clearToken()
      toast("Login failed.", true)
    }
  })

  return root
}

function renderAgents() {
  stopPoll()
  const root = document.createElement("div")
  root.className = "page"
  root.appendChild(nav())

  const main = document.createElement("section")
  main.className = "stack"
  main.innerHTML = `
    <h1>Agents</h1>
    <p class="muted">Select an agent to inspect events and send messages.</p>
    <form id="create-form" class="card inline">
      <input id="create-name" placeholder="new-agent" required />
      <button type="submit">Create</button>
    </form>
    <div id="agents-list" class="stack"></div>
  `

  const list = /** @type {HTMLDivElement} */ (
    main.querySelector("#agents-list")
  )
  if (state.agents.length === 0) {
    list.innerHTML = `<div class="card muted">No agents yet.</div>`
  } else {
    for (const a of state.agents) {
      const row = document.createElement("a")
      row.className = "card row"
      row.href = `#/agents/${encodeURIComponent(a.id)}`
      row.innerHTML = `<strong>${a.name}</strong><span class="muted">${a.status} · ${age(a.created_at)} ago</span>`
      list.appendChild(row)
    }
  }

  main.querySelector("#create-form")?.addEventListener("submit", async e => {
    e.preventDefault()
    const name = /** @type {HTMLInputElement} */ (
      main.querySelector("#create-name")
    ).value.trim()
    if (!name) return
    try {
      await api.createAgent(name)
      await loadAgents()
      render()
      toast(`Created ${name}.`)
    } catch (err) {
      toast(`Create failed: ${err.message || err}`, true)
    }
  })

  root.appendChild(main)
  return root
}

function renderDetail() {
  const a = state.selected
  const root = document.createElement("div")
  root.className = "page"
  root.appendChild(nav())

  if (!a) {
    root.innerHTML += `<section class="stack"><h1>Agent</h1><div class="card muted">Loading...</div></section>`
    return root
  }

  const section = document.createElement("section")
  section.className = "stack"
  section.innerHTML = `
    <div class="inline between">
      <h1>${a.name}</h1>
      <a href="#/agents/${encodeURIComponent(a.id)}/settings">Settings</a>
    </div>
    <div class="card stack">
      <div><strong>Status:</strong> ${a.status}</div>
      <div><strong>Budget:</strong> ${a.budget_used_usd ?? 0} / ${a.budget_limit_usd ?? 0}</div>
      <div><strong>ID:</strong> <code>${a.id}</code></div>
    </div>
    <form id="message-form" class="card stack">
      <label>Message<textarea id="message-input" rows="4" required></textarea></label>
      <button type="submit">Send message</button>
    </form>
    <div class="card stack">
      <h2>Events</h2>
      <div id="events" class="stack"></div>
    </div>
  `

  const eventsEl = /** @type {HTMLDivElement} */ (
    section.querySelector("#events")
  )
  for (const ev of state.events.slice(-100)) {
    const row = document.createElement("div")
    row.className = "event"
    row.innerHTML = `
      <div class="inline between">
        <strong>${ev.event_type}</strong>
        <span class="muted">${age(ev.created_at)} ago</span>
      </div>
      <div class="muted">${ev.source}</div>
      ${ev.content ? `<pre>${ev.content}</pre>` : ""}
      ${ev.tool_output ? `<pre>${ev.tool_output}</pre>` : ""}
      ${ev.termination_reason ? `<div class="muted">termination: ${ev.termination_reason}</div>` : ""}
    `
    eventsEl.appendChild(row)
  }

  section
    .querySelector("#message-form")
    ?.addEventListener("submit", async e => {
      e.preventDefault()
      const msg = /** @type {HTMLTextAreaElement} */ (
        section.querySelector("#message-input")
      ).value.trim()
      if (!msg) return
      try {
        await api.sendMessage(a.id, msg)
        /** @type {HTMLTextAreaElement} */ ;(
          section.querySelector("#message-input")
        ).value = ""
        toast("Message sent.")
      } catch (err) {
        toast(`Send failed: ${err.message || err}`, true)
      }
    })

  root.appendChild(section)
  return root
}

function renderSettings() {
  const a = state.selected
  const root = document.createElement("div")
  root.className = "page"
  root.appendChild(nav())

  if (!a) {
    root.innerHTML += `<section class="stack"><h1>Settings</h1><div class="card muted">Loading...</div></section>`
    return root
  }

  const section = document.createElement("section")
  section.className = "stack"
  section.innerHTML = `
    <div class="inline between">
      <h1>${a.name} Settings</h1>
      <a href="#/agents/${encodeURIComponent(a.id)}">Back</a>
    </div>
    <div class="card stack">
      <h2>Webhook secret</h2>
      <button id="rotate-secret" type="button">Rotate secret</button>
      <pre id="secret-output">${state.rotatingSecret || "(not shown yet)"}</pre>
    </div>
    <form id="budget-form" class="card stack">
      <h2>Budget</h2>
      <label>budget_limit_usd<input id="budget-value" type="number" step="0.000001" value="${a.budget_limit_usd ?? 0}" required /></label>
      <button type="submit">Save budget</button>
      <div class="muted">Current used: ${a.budget_used_usd ?? 0}</div>
    </form>
  `

  section
    .querySelector("#rotate-secret")
    ?.addEventListener("click", async () => {
      try {
        const out = await api.rotateWebhookSecret(a.id)
        state.rotatingSecret = out.webhook_secret
        render()
        toast("Webhook secret rotated.")
      } catch (err) {
        toast(`Rotate failed: ${err.message || err}`, true)
      }
    })

  section.querySelector("#budget-form")?.addEventListener("submit", async e => {
    e.preventDefault()
    const raw = /** @type {HTMLInputElement} */ (
      section.querySelector("#budget-value")
    ).value
    const parsed = Number(raw)
    if (Number.isNaN(parsed)) {
      toast("Invalid budget.", true)
      return
    }
    try {
      await api.setBudget(a.id, parsed)
      state.selected = await api.getAgent(a.id)
      toast("Budget updated.")
      render()
    } catch (err) {
      toast(`Budget update failed: ${err.message || err}`, true)
    }
  })

  root.appendChild(section)
  return root
}

async function render() {
  if (!app) return
  const route = currentRoute()

  try {
    if (route.view !== "login" && !api.getToken()) {
      routeTo("#/login")
      return
    }

    if (route.view === "login") {
      stopPoll()
      app.replaceChildren(renderLogin())
      return
    }

    await loadAgents()

    if (route.view === "agents") {
      stopPoll()
      app.replaceChildren(renderAgents())
      return
    }

    if (route.agentId) {
      await loadDetail(route.agentId)
      if (route.view === "settings") {
        stopPoll()
        app.replaceChildren(renderSettings())
        return
      }
      app.replaceChildren(renderDetail())
      pollEvents(route.agentId).catch(() => {})
      return
    }

    routeTo("#/agents")
  } catch (err) {
    if (err && err.status === 401) {
      api.clearToken()
      routeTo("#/login")
      return
    }
    stopPoll()
    const page = document.createElement("div")
    page.className = "page"
    page.innerHTML = `<div class="card error">Error: ${err?.message || err}</div>`
    app.replaceChildren(page)
  }
}

window.addEventListener("hashchange", () => {
  render().catch(() => {})
})

if (!window.location.hash) {
  routeTo(api.getToken() ? "#/agents" : "#/login")
} else {
  render().catch(() => {})
}
