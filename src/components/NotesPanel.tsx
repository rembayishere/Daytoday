import { useState, useEffect, useRef } from 'react'
import { useAppStore } from '../store/useAppStore'
import { matchShortcut } from '../utils/shortcut'

function NoteItem({ n, activeDateFilter }: { n: import("../types").Note; activeDateFilter: string | null }) {
  const [editing, setEditing] = useState(false)
  const [editText, setEditText] = useState(n.text)
  const attachments = useAppStore((s) => s.data?.attachments || [])
  const noteAttachments = attachments.filter((a) => a.note_ids.includes(n.id))

  const handleAttClick = () => {
    useAppStore.setState({ activePanel: 'files' })
  }

  const handleSave = async () => {
    if (!editText.trim()) return
    await useAppStore.getState().updateNote(n.id, editText.trim())
    setEditing(false)
  }

  const handleDelete = async () => {
    try {
      await useAppStore.getState().deleteNote(n.id)
    } catch (e: any) {
      console.error('删除记录失败', e)
      alert(typeof e === 'string' ? e : (e?.message || '删除记录失败'))
    }
  }

  const handleCancel = () => {
    setEditText(n.text)
    setEditing(false)
  }

  return (
    <div className={`nm${activeDateFilter && n.date === activeDateFilter ? ' filtered-highlight' : ''}`}>
      {editing ? (
        <div style={{flex:1}}>
          <textarea
            value={editText}
            onChange={(e) => setEditText(e.target.value)}
            style={{width:'100%',minHeight:'60px',padding:'8px',border:'1px solid var(--border-input)',borderRadius:'10px',fontSize:'.85rem',fontFamily:'inherit',resize:'vertical',outline:'none',background:'var(--bg-card)',color:'var(--text-primary)'}}
          />
          <div style={{display:'flex',gap:'6px',marginTop:'6px'}}>
            <button className="btn btn-primary" style={{padding:'4px 14px',fontSize:'.72rem'}} onClick={handleSave}>保存</button>
            <button className="btn btn-secondary" style={{padding:'4px 14px',fontSize:'.72rem'}} onClick={handleCancel}>取消</button>
          </div>
        </div>
      ) : (
        <>
          <div className="tx">
            <span>{n.text.split(/(#\S+)/g).map((part: string, i: number) => part.startsWith("#") ? <span key={i} className="tag">{part}</span> : part)}</span>
            <div className="tm">
              {noteAttachments.length > 0 && (
                <span style={{color:'var(--accent)',marginRight:'6px',cursor:'pointer'}} onClick={handleAttClick} title="查看关联资料">
                  <i className="fas fa-paperclip"></i> {noteAttachments.length}
                </span>
              )}
              {n.time}
            </div>
          </div>
          <div className="ac">
            <button onClick={() => { setEditText(n.text); setEditing(true) }} title="编辑"><i className="fas fa-pen"></i></button>
            <button onClick={handleDelete} title="删除"><i className="fas fa-trash"></i></button>
          </div>
        </>
      )}
    </div>
  )
}

export default function NotesPanel() {
  const data = useAppStore((s) => s.data)
  const activeDateFilter = useAppStore((s) => s.activeDateFilter)
  const [text, setText] = useState('')
  const textRef = useRef(text)
  textRef.current = text
  const sendNoteRef = useRef(data?.shortcuts?.send_note || 'Ctrl+Enter')
  sendNoteRef.current = data?.shortcuts?.send_note || 'Ctrl+Enter'

  const handleSend = async () => {
    if (!text.trim()) return
    await useAppStore.getState().addNote(text.trim())
    setText('')
  }

  useEffect(() => {
    const handleKey = (e: KeyboardEvent) => {
      if (matchShortcut(e, sendNoteRef.current)) {
        const val = textRef.current
        if (!val.trim()) return
        useAppStore.getState().addNote(val.trim())
        setText('')
      }
    }
    window.addEventListener('keydown', handleKey)
    return () => window.removeEventListener('keydown', handleKey)
  }, [])

  const notes = data
    ? data.notes.filter((n) => !activeDateFilter || n.date === activeDateFilter)
    : []

  return (
    <div className="pn act" id="panelNotes">
      <div className="ni">
        <textarea
          value={text}
          onChange={(e) => setText(e.target.value)}
          placeholder="写下想法… 支持 #标签"
        />
        <div className="nt" style={{justifyContent:'flex-end'}}>
          <button className="btn btn-primary" onClick={handleSend}>记录</button>
        </div>
      </div>
      <div className="nl">
        {notes.length === 0 ? (
          <div className="em">
            <i className="fas fa-sticky-note" style={{fontSize:'2rem',color:'var(--text-placeholder)'}}></i>
            <div>{activeDateFilter ? '该日期无笔记' : '暂无笔记'}</div>
          </div>
        ) : (
          notes.map((n) => <NoteItem key={n.id} n={n} activeDateFilter={activeDateFilter} />)
        )}
      </div>
    </div>
  )
}
