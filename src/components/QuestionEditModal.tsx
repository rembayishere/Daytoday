import { useState, useEffect } from 'react'
import { useAppStore } from '../store/useAppStore'

export default function QuestionEditModal() {
  const data = useAppStore((s) => s.data)
  const editingQuestionId = useAppStore((s) => s.editingQuestionId)
  const setEditingQuestion = useAppStore((s) => s.setEditingQuestion)
  const updateQuestion = useAppStore((s) => s.updateQuestion)

  const [question, setQuestion] = useState('')
  const [desc, setDesc] = useState('')
  const [tags, setTags] = useState('')

  useEffect(() => {
    if (editingQuestionId && data) {
      const q = data.questions.find((q) => q.id === editingQuestionId)
      if (q) {
        setQuestion(q.question || '')
        setDesc(q.desc || '')
        setTags((q.tags || []).join(', '))
        return
      }
    }
    setQuestion('')
    setDesc('')
    setTags('')
  }, [editingQuestionId, data])

  const handleClose = () => setEditingQuestion(null)

  const handleSave = async () => {
    if (!question.trim() || !editingQuestionId) return
    const tagList = tags.split(',').map((s) => s.trim()).filter(Boolean)
    await updateQuestion(editingQuestionId, question.trim(), desc, tagList.length ? tagList : ['未分类'])
    handleClose()
  }

  useEffect(() => {
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') handleClose()
    }
    if (editingQuestionId) window.addEventListener('keydown', handleKey)
    return () => window.removeEventListener('keydown', handleKey)
  }, [editingQuestionId])

  const isOpen = editingQuestionId !== null

  return (
    <div className={`task-edit-modal${isOpen ? ' show' : ''}`} onClick={(e) => { if (e.target === e.currentTarget) handleClose() }}>
      <div className="task-edit-card">
        <h3>编辑问题</h3>
        <input type="text" value={question} onChange={(e) => setQuestion(e.target.value)} placeholder="问题标题" />
        <textarea
          value={desc}
          onChange={(e) => setDesc(e.target.value)}
          placeholder="描述（可选）"
          style={{width:'100%',padding:'8px 12px',border:'1px solid var(--border-input)',borderRadius:'10px',fontSize:'.85rem',marginBottom:'10px',outline:'none',background:'var(--bg-input)',color:'var(--text-primary)',fontFamily:'inherit',resize:'vertical',minHeight:'60px'}}
        />
        <input type="text" value={tags} onChange={(e) => setTags(e.target.value)} placeholder="标签（逗号分隔）" />
        <div className="btn-row">
          <button className="btn btn-secondary" onClick={handleClose}>取消</button>
          <button className="btn btn-primary" onClick={handleSave}>保存</button>
        </div>
      </div>
    </div>
  )
}
