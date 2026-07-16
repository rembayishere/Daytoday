import { useState } from 'react'
import { useAppStore } from '../store/useAppStore'
import type { Task, TaskPriority, Subtask } from '../types'

const PRIORITY_CLS: Record<TaskPriority, string> = { high: 'h', medium: 'm', low: 'l' }
const COLUMNS = [
  { key: 'todo', label: '📋 待办' },
  { key: 'doing', label: '⏳ 进行中' },
  { key: 'done', label: '✅ 已完成' },
]

function isUrgent(date: string): boolean {
  if (!date) return false
  return date <= new Date().toISOString().slice(0, 10)
}

function futureDate(daysFromNow: number): string {
  const d = new Date()
  d.setDate(d.getDate() + daysFromNow)
  return d.toISOString().slice(0, 10)
}

const QUADRANTS: { key: string; label: string; filter: (t: Task) => boolean }[] = [
  { key: 'do-first', label: '🔥 重要且紧急', filter: (t) => t.priority === 'high' && isUrgent(t.date) },
  { key: 'schedule', label: '📅 重要不紧急', filter: (t) => t.priority === 'high' && !isUrgent(t.date) },
  { key: 'delegate', label: '⚡ 紧急不重要', filter: (t) => t.priority !== 'high' && isUrgent(t.date) },
  { key: 'eliminate', label: '🗑️ 不重要不紧急', filter: (t) => t.priority !== 'high' && !isUrgent(t.date) },
]

function quadrantTarget(key: string): { priority: TaskPriority; date: string } {
  const today = new Date().toISOString().slice(0, 10)
  const future = futureDate(7)
  switch (key) {
    case 'do-first': return { priority: 'high', date: today }
    case 'schedule': return { priority: 'high', date: future }
    case 'delegate': return { priority: 'medium', date: today }
    case 'eliminate': return { priority: 'low', date: future }
    default: return { priority: 'medium', date: future }
  }
}

export default function TaskBoard() {
  const data = useAppStore((s) => s.data)
  const activeDateFilter = useAppStore((s) => s.activeDateFilter)
  const deleteTask = useAppStore((s) => s.deleteTask)
  const moveTask = useAppStore((s) => s.moveTask)
  const updateTask = useAppStore((s) => s.updateTask)
  const setEditingTask = useAppStore((s) => s.setEditingTask)
  const [viewMode, setViewMode] = useState<'kanban' | 'matrix'>('kanban')

  const handleDragStart = (e: React.DragEvent, id: number, from: string) => {
    e.dataTransfer.setData('text/plain', JSON.stringify({ id, from }))
  }

  const handleDrop = async (e: React.DragEvent, to: string) => {
    e.preventDefault()
    e.currentTarget.classList.remove('ov')
    const d = JSON.parse(e.dataTransfer.getData('text/plain'))
    if (d.from !== to) {
      await moveTask(d.id, d.from, to)
    }
  }

  const handleMatrixDragStart = (e: React.DragEvent, id: number, status: string) => {
    e.dataTransfer.setData('text/plain', JSON.stringify({ id, status }))
  }

  const handleMatrixDrop = async (e: React.DragEvent, targetKey: string) => {
    e.preventDefault()
    e.currentTarget.classList.remove('ov')
    const { id: taskId, status } = JSON.parse(e.dataTransfer.getData('text/plain'))
    const all = data ? [...data.tasks.todo, ...data.tasks.doing, ...data.tasks.done] : []
    const task = all.find((t) => t.id === taskId)
    if (!task) return
    const target = quadrantTarget(targetKey)
    if (task.priority === target.priority && task.date === target.date) return
    await updateTask(task.id, status, task.title, target.priority, target.date, task.note || '')
  }

  const renderTask = (t: Task, status: string) => {
    const doneSub = (t.subtasks || []).filter((s: Subtask) => s.done).length
    const totalSub = (t.subtasks || []).length
    return (
      <>
        <span className={`pd ${PRIORITY_CLS[t.priority as TaskPriority]}`}></span>
        <span style={{flex:1,cursor:'pointer'}} onClick={() => setEditingTask(t.id, status)}>{t.title}</span>
        <div style={{display:'flex',alignItems:'center',gap:'4px',flexShrink:0}}>
          {t.note && <span className="task-note-badge" title="有备注"><i className="fas fa-sticky-note"></i></span>}
          {totalSub > 0 && <span className="task-sub-count">{doneSub}/{totalSub}</span>}
        </div>
        <div className="ta">
          <button onClick={(e) => { e.stopPropagation(); setEditingTask(t.id, status) }} title="编辑"><i className="fas fa-pen"></i></button>
          <button onClick={(e) => { e.stopPropagation(); deleteTask(t.id, status) }} title="删除"><i className="fas fa-trash"></i></button>
        </div>
      </>
    )
  }

  if (viewMode === 'matrix') {
    const taskStatusMap = new Map<number, string>()
    if (data) {
      for (const s of ['todo', 'doing', 'done'] as const) {
        for (const t of data.tasks[s]) {
          taskStatusMap.set(t.id, s)
        }
      }
    }
    const allTasks = data ? [...data.tasks.todo, ...data.tasks.doing, ...data.tasks.done] : []
    const filteredTasks = activeDateFilter ? allTasks.filter((t) => t.date === activeDateFilter) : allTasks

    return (
      <div className="pn act">
        <div className="st">
          任务看板
          <button className="btn btn-ghost" style={{marginLeft:'auto',fontSize:'.72rem'}} onClick={() => setViewMode('kanban')}>📋 看板视图</button>
        </div>
        <div className="mx">
          {QUADRANTS.map((q) => {
            const tasks = filteredTasks.filter(q.filter)
            return (
              <div className="mx-q" key={q.key} data-quadrant={q.key}>
                <div className="mx-h">
                  <span>{q.label}</span>
                  <span className="mx-c">{tasks.length}</span>
                </div>
                <div
                  className="mx-b"
                  onDragOver={(e) => { e.preventDefault(); e.currentTarget.classList.add('ov') }}
                  onDragLeave={(e) => e.currentTarget.classList.remove('ov')}
                  onDrop={(e) => handleMatrixDrop(e, q.key)}
                >
                  {tasks.map((t) => {
                    const st = taskStatusMap.get(t.id) || 'todo'
                    return (
                      <div key={t.id} className="tk" draggable onDragStart={(e) => handleMatrixDragStart(e, t.id, st)}>
                        {renderTask(t, st)}
                      </div>
                    )
                  })}
                </div>
              </div>
            )
          })}
        </div>
      </div>
    )
  }

  return (
    <div className="pn act">
      <div className="st">
        任务看板 · 拖拽排序
        <button className="btn btn-ghost" style={{marginLeft:'auto',fontSize:'.72rem'}} onClick={() => setViewMode('matrix')}>📊 矩阵视图</button>
      </div>
      <div className="kb" style={{paddingBottom:70}}>
        {COLUMNS.map((col) => {
          const tasks = data
            ? (data.tasks as import("../types").TaskBoard)[col.key as "todo" | "doing" | "done"].filter((t: any) => !activeDateFilter || t.date === activeDateFilter)
            : []
          return (
            <div className="kc" key={col.key} data-status={col.key}>
              <div className="ch">{col.label} <span style={{fontWeight:400,color:'var(--text-muted)',fontSize:'.7rem'}}>({tasks.length})</span></div>
              <div
                className="cb"
                data-status={col.key}
                onDragOver={(e) => { e.preventDefault(); e.currentTarget.classList.add('ov') }}
                onDragLeave={(e) => e.currentTarget.classList.remove('ov')}
                onDrop={(e) => handleDrop(e, col.key)}
              >
                {tasks.map((t: Task) => (
                  <div
                    key={t.id}
                    className="tk"
                    draggable
                    data-id={t.id}
                    data-status={col.key}
                    onDragStart={(e) => handleDragStart(e, t.id, col.key)}
                  >
                    {renderTask(t, col.key)}
                  </div>
                ))}
              </div>
              <TaskInput colKey={col.key} />
            </div>
          )
        })}
      </div>
    </div>
  )
}

function TaskInput({ colKey }: { colKey: string }) {
  const addTask = useAppStore((s) => s.addTask)
  const [val, setVal] = useState('')

  const handleAdd = async () => {
    if (!val.trim()) return
    await addTask(val.trim(), colKey)
    setVal('')
  }

  return (
    <div className="ca">
      <input
        value={val}
        onChange={(e) => setVal(e.target.value)}
        onKeyDown={(e) => e.key === 'Enter' && handleAdd()}
        placeholder="+ 添加任务"
      />
      <button className="btn btn-primary" onClick={handleAdd}>+</button>
    </div>
  )
}