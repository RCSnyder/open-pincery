// @ts-check
import * as api from "../api.js"
import { loadAgents } from "../state.js"
import { toast } from "../ui.js"

/**
 * @param {(hash: string) => void} go Navigate-and-rerender helper.
 */
export function renderLogin(go) {
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
      go("#/agents")
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
      go("#/agents")
    } catch (_err) {
      api.clearToken()
      toast("Login failed.", true)
    }
  })

  return root
}
