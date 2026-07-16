import { useState } from 'react'
import { useAppStore } from '../store/useAppStore'

function ClipItem({ c }: { c: import("../types").Clip }) {
  const [editing, setEditing] = useState(false)
  const [title, setTitle] = useState(c.title)
  const [url, setUrl] = useState(c.url)

  const handleSave = async () => {
    if (!title.trim()) return
    await useAppStore.getState().updateClip(c.id, title.trim(), url.trim())
    setEditing(false)
  }

  const handleDelete = async () => {
    await useAppStore.getState().deleteClip(c.id)
  }

  if (editing) {
    return (
      <div className="cp" style={{width:'260px'}}>
        <input value={title} onChange={(e) => setTitle(e.target.value)} placeholder="标题" style={{width:'100%',padding:'6px 8px',border:'1px solid var(--border-input)',borderRadius:'10px',fontSize:'.78rem',marginBottom:'6px',outline:'none',background:'var(--bg-input)',color:'var(--text-primary)'}} />
        <input value={url} onChange={(e) => setUrl(e.target.value)} placeholder="URL" style={{width:'100%',padding:'6px 8px',border:'1px solid var(--border-input)',borderRadius:'10px',fontSize:'.78rem',marginBottom:'6px',outline:'none',background:'var(--bg-input)',color:'var(--text-primary)'}} />
        <div style={{display:'flex',gap:'6px'}}>
          <button className="btn btn-primary" style={{padding:'4px 14px',fontSize:'.72rem'}} onClick={handleSave}>保存</button>
          <button className="btn btn-secondary" style={{padding:'4px 14px',fontSize:'.72rem'}} onClick={() => setEditing(false)}>取消</button>
        </div>
      </div>
    )
  }

  return (
    <div className="cp" style={{position:'relative'}}>
      <div className="ta" style={{position:'absolute',top:'5px',right:'6px',gap:'2px'}}>
        <button onClick={() => { setTitle(c.title); setUrl(c.url); setEditing(true) }} title="编辑"><i className="fas fa-pen"></i></button>
        <button onClick={handleDelete} title="删除"><i className="fas fa-trash"></i></button>
      </div>
      <div className="cv"><i className="fas fa-globe"></i></div>
      <strong>{c.title}</strong>
      <div style={{fontSize:'.7rem',color:'var(--text-muted)'}}>{c.url}</div>
    </div>
  )
}

function AddClipForm() {
  const [title, setTitle] = useState('')
  const [url, setUrl] = useState('')
  const [open, setOpen] = useState(false)

  const handleAdd = async () => {
    if (!title.trim() || !url.trim()) return
    await useAppStore.getState().addClip(title.trim(), url.trim())
    setTitle('')
    setUrl('')
    setOpen(false)
  }

  if (!open) {
    return <button className="btn btn-secondary" onClick={() => setOpen(true)} style={{marginBottom:'10px'}}><i className="fas fa-plus"></i> 添加剪藏</button>
  }

  return (
    <div className="cp" style={{width:'260px',marginBottom:'10px'}}>
      <input value={title} onChange={(e) => setTitle(e.target.value)} placeholder="标题" style={{width:'100%',padding:'6px 8px',border:'1px solid var(--border-input)',borderRadius:'10px',fontSize:'.78rem',marginBottom:'6px',outline:'none',background:'var(--bg-input)',color:'var(--text-primary)'}} />
      <input value={url} onChange={(e) => setUrl(e.target.value)} placeholder="URL" style={{width:'100%',padding:'6px 8px',border:'1px solid var(--border-input)',borderRadius:'10px',fontSize:'.78rem',marginBottom:'6px',outline:'none',background:'var(--bg-input)',color:'var(--text-primary)'}} />
      <div style={{display:'flex',gap:'6px'}}>
        <button className="btn btn-primary" style={{padding:'4px 14px',fontSize:'.72rem'}} onClick={handleAdd}>添加</button>
        <button className="btn btn-secondary" style={{padding:'4px 14px',fontSize:'.72rem'}} onClick={() => setOpen(false)}>取消</button>
      </div>
    </div>
  )
}

export default function ClipsPanel() {
  const data = useAppStore((s) => s.data)

  return (
    <div className="pn act">
      <div className="st">剪藏</div>
      <AddClipForm />
      <div className="cg">
        {(!data?.clips || data.clips.length === 0) ? (
          <div className="em">暂无剪藏</div>
        ) : (
          data.clips.map((c) => <ClipItem key={c.id} c={c} />)
        )}
      </div>
    </div>
  )
}
