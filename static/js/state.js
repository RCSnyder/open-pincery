// @ts-check
import * as api from "./api.js"

/** @typedef {{id:string,name:string,status:string,created_at:string,budget_limit_usd?:number,budget_used_usd?:number}} Agent */
/** @typedef {{id:string,event_type:string,source:string,content?:string,tool_name?:string,tool_output?:string,termination_reason?:string,created_at:string}} EventRow */

/** @type {{agents: Agent[], selected: Agent|null, events: EventRow[], pollSince: string|null, pollDelayMs: number, pollTimer: number|null, rotatingSecret: string|null, rotatingSecretAgentId: string|null}} */
export const state = {
  agents: [],
  selected: null,
  events: [],
  pollSince: null,
  pollDelayMs: 4000,
  pollTimer: null,
  rotatingSecret: null,
  rotatingSecretAgentId: null
}

export function stopPoll() {
  if (state.pollTimer !== null) {
    window.clearTimeout(state.pollTimer)
    state.pollTimer = null
  }
}

export function clearRotatedSecret() {
  state.rotatingSecret = null
  state.rotatingSecretAgentId = null
}

/** @returns {Promise<void>} */
export async function loadAgents() {
  state.agents = await api.listAgents()
}

/** @param {string} agentId */
export async function loadDetail(agentId) {
  if (state.rotatingSecretAgentId !== agentId) {
    clearRotatedSecret()
  }

  state.selected = await api.getAgent(agentId)
  const first = await api.getEvents(agentId, { limit: 200 })
  const initialEvents = Array.isArray(first.events)
    ? first.events.slice().reverse()
    : []
  state.events = initialEvents
  state.pollSince =
    initialEvents.length > 0 ? initialEvents[initialEvents.length - 1].id : null
  state.pollDelayMs = 4000
}

/** @param {string} agentId @param {() => void} onUpdate */
export async function pollEvents(agentId, onUpdate) {
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
      onUpdate()
    }
    state.pollDelayMs = 4000
  } catch (_e) {
    state.pollDelayMs = Math.min(state.pollDelayMs * 2, 32000)
  }
  state.pollTimer = window.setTimeout(() => {
    pollEvents(agentId, onUpdate).catch(() => {})
  }, state.pollDelayMs)
}
