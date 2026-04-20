// @ts-check
import * as api from "../api.js"
import { state } from "../state.js"
import { nav, toast } from "../ui.js"

/**
 * @param {() => void} onLogout
 * @param {() => void} rerender
 */
export function renderSettings(onLogout, rerender) {
  const a = state.selected
  const root = document.createElement("div")
  root.className = "page"
  root.appendChild(nav(onLogout))

  if (!a) {
    root.innerHTML += `<section class="stack"><h1>Settings</h1><div class="card muted">Loading...</div></section>`
    return root
  }

  const section = document.createElement("section")
  section.className = "stack"
  section.innerHTML = `
    <div class="inline between">
      <h1 id="settings-name"></h1>
      <a id="settings-back-link">Back</a>
    </div>
    <div class="card stack">
      <h2>Webhook secret</h2>
      <button id="rotate-secret" type="button">Rotate secret</button>
      <pre id="secret-output"></pre>
    </div>
    <form id="budget-form" class="card stack">
      <h2>Budget</h2>
      <label>budget_limit_usd<input id="budget-value" type="number" step="0.000001" required /></label>
      <button type="submit">Save budget</button>
      <div id="budget-used" class="muted"></div>
    </form>
  `

  const settingsName = /** @type {HTMLHeadingElement} */ (
    section.querySelector("#settings-name")
  )
  settingsName.textContent = `${a.name} Settings`

  const settingsBackLink = /** @type {HTMLAnchorElement} */ (
    section.querySelector("#settings-back-link")
  )
  settingsBackLink.href = `#/agents/${encodeURIComponent(a.id)}`

  const secretOutput = /** @type {HTMLElement} */ (
    section.querySelector("#secret-output")
  )
  secretOutput.textContent =
    state.rotatingSecretAgentId === a.id
      ? state.rotatingSecret || "(not shown yet)"
      : "(not shown yet)"

  const budgetValue = /** @type {HTMLInputElement} */ (
    section.querySelector("#budget-value")
  )
  budgetValue.value = String(a.budget_limit_usd ?? 0)

  const budgetUsed = /** @type {HTMLDivElement} */ (
    section.querySelector("#budget-used")
  )
  budgetUsed.textContent = `Current used: ${a.budget_used_usd ?? 0}`

  section
    .querySelector("#rotate-secret")
    ?.addEventListener("click", async () => {
      try {
        const out = await api.rotateWebhookSecret(a.id)
        state.rotatingSecret = out.webhook_secret
        state.rotatingSecretAgentId = a.id
        rerender()
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
      rerender()
    } catch (err) {
      toast(`Budget update failed: ${err.message || err}`, true)
    }
  })

  root.appendChild(section)
  return root
}
