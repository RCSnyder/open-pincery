// @ts-check
import * as api from "../api.js"
import { state, loadAgents } from "../state.js"
import { age, nav, toast } from "../ui.js"

/**
 * @param {() => void} onLogout
 * @param {() => void} rerender
 */
export function renderAgents(onLogout, rerender) {
  const root = document.createElement("div")
  root.className = "page"
  root.appendChild(nav(onLogout))

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
    const empty = document.createElement("div")
    empty.className = "card muted"
    empty.textContent = "No agents yet."
    list.appendChild(empty)
  } else {
    for (const a of state.agents) {
      const row = document.createElement("a")
      row.className = "card row"
      row.href = `#/agents/${encodeURIComponent(a.id)}`
      const name = document.createElement("strong")
      name.textContent = a.name
      const meta = document.createElement("span")
      meta.className = "muted"
      meta.textContent = `${a.status} · ${age(a.created_at)} ago`
      row.append(name, meta)
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
      rerender()
      toast(`Created ${name}.`)
    } catch (err) {
      toast(`Create failed: ${err.message || err}`, true)
    }
  })

  root.appendChild(main)
  return root
}
