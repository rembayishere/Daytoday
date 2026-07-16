import { useState, useEffect, useRef } from 'react'
import { useAppStore } from '../store/useAppStore'

function todayLocal(): string {
  const d = new Date()
  const y = d.getFullYear()
  const m = String(d.getMonth() + 1).padStart(2, '0')
  const day = String(d.getDate()).padStart(2, '0')
  return `${y}-${m}-${day}`
}

export default function TaskEditModal() {
  const data = useAppStore((s) => s.data)
  const { id, status } = useAppStore((s) => s.editingTask)
  const setEditingTask = useAppStore((s) => s.setEditingTask)
  const updateTask = useAppStore((s) => s.updateTask)
  const addTask = useAppStore((s) => s.addTask)
  const addTaskSubtask = useAppStore((s) => s.addTaskSubtask)
  const toggleTaskSubtask = useAppStore((s) => s.toggleTaskSubtask)
  const deleteTaskSubtask = useAppStore((s) => s.deleteTaskSubtask)

  const [title, setTitle] = useState('')
  const [priority, setPriority] = useState('medium')
  const [date, setDate] = useState('')
  const [note, setNote] = useState('')
  const [subtasks, setSubtasks] = useState<{ id: number; text: string; done: boolean }[]>([])
  const [newSub, setNewSub] = useState('')
  const noteTimer = useRef<any>(null)

  useEffect(() => {
    if (id && status && data) {
      const list = (data.tasks as import("../types").TaskBoard)[status as "todo" | "doing" | "done"] || []
      const task = list.find((t: import("../types").Task) => t.id === id)
      if (task) {
        setTitle(task.title)
        setPriority(task.priority)
        setDate(task.date || todayLocal())
        setNote(task.note || '')
        setSubtasks(task.subtasks || [])
        return
      }
    }
    setTitle('')
    setPriority('medium')
    setDate(todayLocal())
    setNote('')
    setSubtasks([])
  }, [id, status, data])

  const isOpen = id !== null || status !== null

  useEffect(() => {
    return () => { if (noteTimer.current) clearTimeout(noteTimer.current) }
  }, [])

  const handleClose = () => setEditingTask(null, null)

  const handleSave = async () => {
    if (!title.trim()) return
    const d = date || todayLocal()
    if (id && status) {
      await updateTask(id, status, title.trim(), priority, d, note)
    } else if (status) {
      await addTask(title.trim(), status)
    }
    handleClose()
  }

  const handleNoteChange = (val: string) => {
    setNote(val)
    if (noteTimer.current) clearTimeout(noteTimer.current)
    noteTimer.current = setTimeout(async () => {
      if (id && status) {
        await updateTask(id, status, title, priority, date || todayLocal(), val)
      }
    }, 500)
  }

  const handleAddSub = async () => {
    if (!newSub.trim() || !id || !status) return
    await addTaskSubtask(id, status, newSub.trim())
    setNewSub('')
    const list = (data!.tasks as any)[status] || []
    const task = list.find((t: import("../types").Task) => t.id === id)
    if (task) setSubtasks(task.subtasks || [])
  }

  const handleToggleSub = async (subId: number) => {
    if (!id || !status) return
    await toggleTaskSubtask(id, status, subId)
    const list = (data!.tasks as any)[status] || []
    const task = list.find((t: any) => t.id === id)
    if (task) setSubtasks(task.subtasks || [])
  }

  const handleDeleteSub = async (subId: number) => {
    if (!id || !status) return
    await deleteTaskSubtask(id, status, subId)
    const list = (data!.tasks as any)[status] || []
    const task = list.find((t: any) => t.id === id)
    if (task) setSubtasks(task.subtasks || [])
  }

  useEffect(() => {
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') handleClose()
    }
    if (isOpen) window.addEventListener('keydown', handleKey)
    return () => window.removeEventListener('keydown', handleKey)
  }, [isOpen])

  return (
    <div className={`task-edit-modal${isOpen ? ' show' : ''}`} onClick={(e) => { if (e.target === e.currentTarget) handleClose() }}>
      <div className="task-edit-card">
        <h3>{id ? '编辑任务' : '新建任务'}</h3>
        <input type="text" value={title} onChange={(e) => setTitle(e.target.value)} placeholder="任务名称" />
        <select value={priority} onChange={(e) => setPriority(e.target.value)}>
          <option value="high">🔴 高优先级</option>
          <option value="medium">🟡 中优先级</option>
          <option value="low">🟢 低优先级</option>
        </select>
        <input type="text" value={date} onChange={(e) => setDate(e.target.value)} placeholder="日期 (YYYY-MM-DD)，默认今天" />
        <div style={{marginBottom:'10px'}}>
          <label style={{fontSize:'.78rem',color:'var(--text-secondary)',display:'block',marginBottom:'4px'}}>备注</label>
          <textarea
            value={note}
            onChange={(e) => handleNoteChange(e.target.value)}
            placeholder="备注..."
            rows={2}
            style={{width:'100%',border:'1px solid var(--border-input)',borderRadius:'10px',padding:'8px',fontSize:'.82rem',fontFamily:'inherit',lineHeight:1.5,resize:'vertical',outline:'none',background:'var(--bg-card)',color:'var(--text-primary)'}}
          />
        </div>
          {id && (
          <div style={{marginBottom:'10px'}}>
            <label style={{fontSize:'.78rem',color:'var(--text-secondary)',display:'block',marginBottom:'4px'}}>子任务</label>
            {subtasks.length > 0 && (
              <div className="tsub" style={{marginBottom:'6px'}}>
                {subtasks.map((s) => (
                  <div className="tsub-item" key={s.id}>
                    <input type="checkbox" checked={s.done} onChange={() => handleToggleSub(s.id)} style={{cursor:'pointer',accentColor:'var(--accent)'}} />
                    <span className={s.done ? 'tsub-done' : ''} style={{fontSize:'.78rem',color:'var(--text-primary)'}}>{s.text}</span>
                    <button className="tsub-del" onClick={() => handleDeleteSub(s.id)}><i className="fas fa-times"></i></button>
                  </div>
                ))}
              </div>
            )}
            <div className="tsub-add">
              <input value={newSub} onChange={(e) => setNewSub(e.target.value)} onKeyDown={(e) => e.key === 'Enter' && handleAddSub()} placeholder="添加子任务..." />
              <button className="btn btn-primary" style={{padding:'3px 10px',fontSize:'.7rem'}} onClick={handleAddSub}>添加</button>
            </div>
          </div>
        )}
        <div className="btn-row">
          <button className="btn btn-secondary" onClick={handleClose}>取消</button>
          <button className="btn btn-primary" onClick={handleSave}>保存</button>
        </div>
      </div>
    </div>
  )
}
