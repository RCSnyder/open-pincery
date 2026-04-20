// @ts-check

/** @param {string} msg @param {boolean=} isError */
export function toast(msg, isError = false) {
  const old = document.getElementById("toast")
  if (old) old.remove()
  const el = document.createElement("div")
  el.id = "toast"
  el.className = `toast ${isError ? "error" : "ok"}`
  el.textContent = msg
  document.body.appendChild(el)
  window.setTimeout(() => el.remove(), 3000)
}

/** @param {string} iso */
export function age(iso) {
  const sec = Math.max(
    0,
    Math.floor((Date.now() - new Date(iso).getTime()) / 1000)
  )
  if (sec < 60) return `${sec}s`
  if (sec < 3600) return `${Math.floor(sec / 60)}m`
  if (sec < 86400) return `${Math.floor(sec / 3600)}h`
  return `${Math.floor(sec / 86400)}d`
}

export function currentRoute() {
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

/**
 * Change the URL hash. If already there, trigger `onSame` so the caller can
 * force a re-render (hashchange won't fire when the hash is unchanged).
 * @param {string} hash
 * @param {() => void} onSame
 */
export function routeTo(hash, onSame) {
  if (window.location.hash !== hash) {
    window.location.hash = hash
    return
  }
  onSame()
}

/** @param {() => void} onLogout */
export function nav(onLogout) {
  const wrap = document.createElement("nav")
  wrap.className = "topnav"
  wrap.innerHTML = `
    <a href="#/agents">Agents</a>
    <a href="#/login">Login</a>
    <button id="logout-btn" type="button">Logout</button>
  `
  wrap.querySelector("#logout-btn")?.addEventListener("click", onLogout)
  return wrap
}
