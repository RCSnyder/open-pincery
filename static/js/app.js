// @ts-check
import * as api from "./api.js"
import {
  clearRotatedSecret,
  loadAgents,
  loadDetail,
  pollEvents,
  state,
  stopPoll
} from "./state.js"
import { currentRoute, routeTo } from "./ui.js"
import { renderAgents } from "./views/agents.js"
import { renderDetail } from "./views/detail.js"
import { renderLogin } from "./views/login.js"
import { renderSettings } from "./views/settings.js"

const app = document.getElementById("app")

/** Navigate to a hash. If already there, force a re-render. */
function go(hash) {
  routeTo(hash, () => {
    render().catch(() => {})
  })
}

function onLogout() {
  api.clearToken()
  stopPoll()
  clearRotatedSecret()
  go("#/login")
}

function rerender() {
  render().catch(() => {})
}

async function render() {
  if (!app) return
  const route = currentRoute()

  try {
    if (route.view !== "login" && !api.getToken()) {
      go("#/login")
      return
    }

    if (route.view === "login") {
      stopPoll()
      app.replaceChildren(renderLogin(go))
      return
    }

    await loadAgents()

    if (route.view === "agents") {
      stopPoll()
      app.replaceChildren(renderAgents(onLogout, rerender))
      return
    }

    if (route.agentId) {
      await loadDetail(route.agentId)
      if (route.view === "settings") {
        stopPoll()
        app.replaceChildren(renderSettings(onLogout, rerender))
        return
      }
      app.replaceChildren(renderDetail(onLogout))
      pollEvents(route.agentId, rerender).catch(() => {})
      return
    }

    go("#/agents")
  } catch (err) {
    if (err && err.status === 401) {
      api.clearToken()
      clearRotatedSecret()
      go("#/login")
      return
    }
    stopPoll()
    const page = document.createElement("div")
    page.className = "page"
    const card = document.createElement("div")
    card.className = "card error"
    card.textContent = `Error: ${String(err?.message ?? err ?? "Unknown error")}`
    page.appendChild(card)
    app.replaceChildren(page)
  }
}

window.addEventListener("hashchange", () => {
  render().catch(() => {})
})

if (!window.location.hash) {
  go(api.getToken() ? "#/agents" : "#/login")
} else {
  render().catch(() => {})
}

// `state` is re-exported so smoke tests and debuggers can inspect the store.
export { state }
