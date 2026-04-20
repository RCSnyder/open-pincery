// @ts-check
import * as api from "../api.js"
import { state } from "../state.js"
import { age, nav, toast } from "../ui.js"

/** @param {() => void} onLogout */
export function renderDetail(onLogout) {
  const a = state.selected
  const root = document.createElement("div")
  root.className = "page"
  root.appendChild(nav(onLogout))

  if (!a) {
    root.innerHTML += `<section class="stack"><h1>Agent</h1><div class="card muted">Loading...</div></section>`
    return root
  }

  const section = document.createElement("section")
  section.className = "stack"
  section.innerHTML = `
    <div class="inline between">
      <h1 id="agent-name"></h1>
      <a id="agent-settings-link">Settings</a>
    </div>
    <div class="card stack">
      <div><strong>Status:</strong> <span id="agent-status"></span></div>
      <div><strong>Budget:</strong> <span id="agent-budget"></span></div>
      <div><strong>ID:</strong> <code id="agent-id"></code></div>
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

  const agentName = /** @type {HTMLHeadingElement} */ (
    section.querySelector("#agent-name")
  )
  agentName.textContent = a.name

  const settingsLink = /** @type {HTMLAnchorElement} */ (
    section.querySelector("#agent-settings-link")
  )
  settingsLink.href = `#/agents/${encodeURIComponent(a.id)}/settings`

  const agentStatus = /** @type {HTMLSpanElement} */ (
    section.querySelector("#agent-status")
  )
  agentStatus.textContent = a.status

  const agentBudget = /** @type {HTMLSpanElement} */ (
    section.querySelector("#agent-budget")
  )
  agentBudget.textContent = `${a.budget_used_usd ?? 0} / ${a.budget_limit_usd ?? 0}`

  const agentId = /** @type {HTMLElement} */ (
    section.querySelector("#agent-id")
  )
  agentId.textContent = a.id

  const eventsEl = /** @type {HTMLDivElement} */ (
    section.querySelector("#events")
  )
  for (const ev of state.events.slice(-100)) {
    const row = document.createElement("div")
    row.className = "event"

    const header = document.createElement("div")
    header.className = "inline between"

    const eventType = document.createElement("strong")
    eventType.textContent = ev.event_type

    const eventAge = document.createElement("span")
    eventAge.className = "muted"
    eventAge.textContent = `${age(ev.created_at)} ago`

    header.append(eventType, eventAge)
    row.appendChild(header)

    const source = document.createElement("div")
    source.className = "muted"
    source.textContent = ev.source
    row.appendChild(source)

    if (ev.content) {
      const content = document.createElement("pre")
      content.textContent = ev.content
      row.appendChild(content)
    }

    if (ev.tool_output) {
      const toolOutput = document.createElement("pre")
      toolOutput.textContent = ev.tool_output
      row.appendChild(toolOutput)
    }

    if (ev.termination_reason) {
      const termination = document.createElement("div")
      termination.className = "muted"
      termination.textContent = `termination: ${ev.termination_reason}`
      row.appendChild(termination)
    }

    eventsEl.appendChild(row)
  }

  section
    .querySelector("#message-form")
    ?.addEventListener("submit", async e => {
      e.preventDefault()
      const input = /** @type {HTMLTextAreaElement} */ (
        section.querySelector("#message-input")
      )
      const msg = input.value.trim()
      if (!msg) return
      try {
        await api.sendMessage(a.id, msg)
        input.value = ""
        toast("Message sent.")
      } catch (err) {
        toast(`Send failed: ${err.message || err}`, true)
      }
    })

  root.appendChild(section)
  return root
}
