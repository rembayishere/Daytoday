import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import App from './App.tsx'

const savedTheme = (() => {
  try { const t = localStorage.getItem('flomo_plus_theme'); if (t === 'blue' || t === 'indigo') return t } catch {}
  return 'blue'
})()
document.documentElement.setAttribute('data-theme', savedTheme)

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
