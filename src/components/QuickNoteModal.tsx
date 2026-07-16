import { useState, useEffect, useRef } from 'react'
import { useAppStore } from '../store/useAppStore'
import { matchShortcut } from '../utils/shortcut'

export default function QuickNoteModal() {
  const data = useAppStore((s) => s.data)
  const addNote = useAppStore((s) => s.addNote)
  const [text, setText] = useState('')
  const quickNoteRef = useRef(data?.shortcuts?.quick_note || 'Alt+Q')
  quickNoteRef.current = data?.shortcuts?.quick_note || 'Alt+Q'

  const handleClose = () => {
    document.getElementById('quickModal')?.classList.remove('op')
  }

  const handleSend = async () => {
    if (!text.trim()) return
    await addNote(text.trim())
    setText('')
    handleClose()
  }

  useEffect(() => {
    const handleKey = (e: KeyboardEvent) => {
      if (matchShortcut(e, quickNoteRef.current)) {
        e.preventDefault()
        const modal = document.getElementById('quickModal')
        if (modal) {
          modal.classList.toggle('op')
          if (modal.classList.contains('op')) {
            setTimeout(() => document.getElementById('quickInput')?.focus(), 50)
          }
        }
      } else if (e.key === 'Escape') {
        document.getElementById('quickModal')?.classList.remove('op')
      }
    }
    window.addEventListener('keydown', handleKey)
    return () => window.removeEventListener('keydown', handleKey)
  }, [])

  return (
    <div className="mo" id="quickModal" onClick={(e) => { if (e.target === e.currentTarget) handleClose() }}>
      <div className="mb">
        <h3>快速记录</h3>
        <textarea id="quickInput" value={text} onChange={(e) => setText(e.target.value)} placeholder="写点什么…" />
        <div className="mf">
          <span className="h">Alt+Enter 发送</span>
          <button className="btn btn-primary" id="quickSend" onClick={handleSend}>发送</button>
        </div>
      </div>
    </div>
  )
}
