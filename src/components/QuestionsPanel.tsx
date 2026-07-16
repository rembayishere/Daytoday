import { useState, useEffect, useRef } from 'react'
import { useAppStore } from '../store/useAppStore'
import type { Question, QuestionStatus } from '../types'
import { QUESTION_STATUS_LABEL } from '../types'

function stCls(s: QuestionStatus) {
  return s === 'open' ? 's0' : s === 'in-progress' ? 's2' : 's1'
}
function stIcon(s: QuestionStatus) {
  return s === 'open' ? 'fa-circle' : s === 'in-progress' ? 'fa-spinner' : 'fa-check-circle'
}

export default function QuestionsPanel() {
  const data = useAppStore((s) => s.data)
  const activeDateFilter = useAppStore((s) => s.activeDateFilter)
  const addQuestion = useAppStore((s) => s.addQuestion)
  const [newQ, setNewQ] = useState('')

  const handleAdd = async () => {
    if (!newQ.trim()) return
    const tags: string[] = []
    const m = newQ.match(/#(\S+)/g)
    if (m) tags.push(...m.map((x) => x.slice(1)))
    if (tags.length === 0) tags.push('未分类')
    await addQuestion(newQ.trim(), '', tags)
    setNewQ('')
  }

  const questions = data
    ? data.questions.filter((q) => !activeDateFilter || q.date === activeDateFilter)
    : []

  const stats = data
    ? { open: data.questions.filter((q) => q.status === 'open').length,
        progress: data.questions.filter((q) => q.status === 'in-progress').length,
        answered: data.questions.filter((q) => q.status === 'answered').length }
    : { open: 0, progress: 0, answered: 0 }

  return (
    <div className="pn act">
      <div className="st">问题追踪</div>
      <div className="qst">
        <span className="sb2"><i className="fas fa-circle" style={{color:'var(--status-open-text)',fontSize:'.6rem'}}></i> 待解决 {stats.open}</span>
        <span className="sb2"><i className="fas fa-spinner" style={{color:'var(--status-progress-text)',fontSize:'.6rem'}}></i> 进行中 {stats.progress}</span>
        <span className="sb2"><i className="fas fa-check-circle" style={{color:'var(--status-answered-text)',fontSize:'.6rem'}}></i> 已解答 {stats.answered}</span>
      </div>
      <div className="aq">
        <input value={newQ} onChange={(e) => setNewQ(e.target.value)} onKeyDown={(e) => e.key === 'Enter' && handleAdd()} placeholder="记录一个新问题..." />
        <button className="btn btn-primary" onClick={handleAdd}>记录</button>
      </div>
      {questions.length === 0 ? (
        <div className="em">{activeDateFilter ? '该日期无问题' : '暂无问题'}</div>
      ) : (
        <div className="ql">
          {questions.map((q) => (
            <QuestionCard key={q.id} q={q} />
          ))}
        </div>
      )}
    </div>
  )
}

function QuestionCard({ q }: { q: Question }) {
  const cycleQuestion = useAppStore((s) => s.cycleQuestion)
  const updateQuestionNote = useAppStore((s) => s.updateQuestionNote)
  const setEditingQuestion = useAppStore((s) => s.setEditingQuestion)
  const deleteQuestion = useAppStore((s) => s.deleteQuestion)
  const [tlOpen, setTlOpen] = useState(false)
  const noteTimer = useRef<ReturnType<typeof setTimeout> | null>(null)

  const handleNoteChange = (val: string) => {
    if (noteTimer.current) clearTimeout(noteTimer.current)
    noteTimer.current = setTimeout(async () => {
      try {
        await updateQuestionNote(q.id, val)
      } catch (e) {
        console.error('更新备注失败', e)
      }
    }, 500)
  }

  useEffect(() => {
    return () => { if (noteTimer.current) clearTimeout(noteTimer.current) }
  }, [])

  const handleCycle = async () => {
    try {
      await cycleQuestion(q.id)
    } catch (e) {
      console.error('推进状态失败', e)
    }
  }

  const handleDelete = async () => {
    try {
      await deleteQuestion(q.id)
    } catch (e) {
      console.error('删除问题失败', e)
    }
  }

  const si = { c: stCls(q.status), i: stIcon(q.status) }

  return (
    <div className={`qc${q.filtered ? ' filtered-highlight' : ''}`}>
      <div className="qh">
        <span className="qi"><i className="fas fa-question-circle"></i></span>
        <div className="qt">{q.question}</div>
        <span className={`qs ${si.c}`} onClick={handleCycle} style={{cursor:'pointer'}}>
          <i className={`fas ${si.i}`}></i> {QUESTION_STATUS_LABEL[q.status]}
        </span>
      </div>
      <div className="qd">{q.desc || ''}</div>
      <div className="qn">
        <textarea
          defaultValue={q.note || ''}
          rows={1}
          onChange={(e) => handleNoteChange(e.target.value)}
          placeholder="备注..."
          onInput={(e) => {
            const el = e.currentTarget
            el.style.height = 'auto'
            el.style.height = Math.max(30, el.scrollHeight) + 'px'
          }}
        />
      </div>
      <div className="qm">
        {q.tags.map((t: string) => (
          <span key={t} className="qtg" onClick={() => setEditingQuestion(q.id)} style={{cursor:'pointer'}}>#{t}</span>
        ))}
        <span><i className="far fa-clock"></i> {q.created}</span>
      </div>
      <div className="qa">
        <button className="btn btn-secondary" onClick={handleCycle}><i className="fas fa-arrow-right"></i> 推进</button>
        <button className="btn btn-secondary" onClick={() => setEditingQuestion(q.id)}><i className="fas fa-pen"></i> 编辑</button>
        <button className="btn btn-secondary" onClick={handleDelete}><i className="fas fa-trash"></i> 删除</button>
        <button className="btn btn-secondary" onClick={() => setTlOpen(!tlOpen)}><i className="fas fa-history"></i> 时间线</button>
      </div>
      {tlOpen && <Timeline q={q} />}
    </div>
  )
}


function htmlEscape(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;").replace(/'/g, "&#39;");
}

function Timeline({ q }: { q: Question }) {
  const [selA, setSelA] = useState(0)
  const [selB, setSelB] = useState(q.versions ? Math.max(0, q.versions.length - 1) : 0)
  const [diffHtml, setDiffHtml] = useState('')

  if (!q.versions || q.versions.length === 0) {
    return <div className="tl op" style={{display:'block'}}><div className="em">暂无版本记录</div></div>
  }

  const handleCompare = () => {
    const a = q.versions[selA]
    const b = q.versions[selB]
    if (!a || !b) return
    let html = '<div style="padding:6px 10px;background:var(--bg-sidebar);border-radius:8px;font-size:.72rem;line-height:1.5;font-family:monospace;">'
    let hasDiff = false
    if (a.question !== b.question) {
      html += `标题：<span style="color:var(--status-open-text);background:var(--status-open-bg);text-decoration:line-through;">${htmlEscape(a.question||'(空)')}</span> → <span style="color:var(--status-answered-text);background:var(--status-answered-bg);">${htmlEscape(b.question||'(空)')}</span><br>`
      hasDiff = true
    }
    if (a.status !== b.status) {
      html += `状态：<span style="color:var(--status-open-text);background:var(--status-open-bg);text-decoration:line-through;">${QUESTION_STATUS_LABEL[a.status as keyof typeof QUESTION_STATUS_LABEL]}</span> → <span style="color:var(--status-answered-text);background:var(--status-answered-bg);">${QUESTION_STATUS_LABEL[b.status as keyof typeof QUESTION_STATUS_LABEL]}</span><br>`
      hasDiff = true
    }
    if (a.note !== b.note) {
      html += `备注：<span style="color:var(--status-open-text);background:var(--status-open-bg);text-decoration:line-through;">${htmlEscape(a.note||'(空)')}</span> → <span style="color:var(--status-answered-text);background:var(--status-answered-bg);">${htmlEscape(b.note||'(空)')}</span><br>`
      hasDiff = true
    }
    if (JSON.stringify(a.tags) !== JSON.stringify(b.tags)) {
      html += `标签：<span style="color:var(--status-open-text);background:var(--status-open-bg);text-decoration:line-through;">[${htmlEscape(a.tags.join(', '))}]</span> → <span style="color:var(--status-answered-text);background:var(--status-answered-bg);">[${htmlEscape(b.tags.join(', '))}]</span><br>`
      hasDiff = true
    }
    if (!hasDiff) html += '<span style="color:var(--text-muted);">无差异</span>'
    html += '</div>'
    setDiffHtml(html)
  }

  return (
    <div className="tl op" style={{display:'block'}}>
      <div style={{display:'flex',gap:'6px',alignItems:'center',marginBottom:'8px'}}>
        <select value={selA} onChange={(e) => setSelA(Number(e.target.value))}>
          {q.versions.map((v, i: number) => (
            <option key={v.id} value={i}>v{i+1} {v.timestamp.slice(0,10)}</option>
          ))}
        </select>
        <span style={{fontSize:'.7rem',color:'var(--text-muted)'}}>↔</span>
        <select value={selB} onChange={(e) => setSelB(Number(e.target.value))}>
          {q.versions.map((v, i: number) => (
            <option key={v.id} value={i}>v{i+1} {v.timestamp.slice(0,10)}</option>
          ))}
        </select>
        <button className="btn btn-secondary" style={{padding:'2px 10px'}} onClick={handleCompare}>比对</button>
      </div>
      <div dangerouslySetInnerHTML={{ __html: diffHtml }} />
      {[...q.versions].reverse().map((v: import("../types").Version, i: number) => (
        <div className="ti" key={v.id}>
          <div className="ts">v{q.versions.length - i} · {v.timestamp.slice(0,10)} {v.timestamp.slice(11,16)}</div>
          <div className="ac">
            {v.change_desc || '创建问题'}
            <span className={`qs ${stCls(v.status)}`} style={{fontSize:'.6rem',padding:'1px 8px',display:'inline-flex',marginLeft:'4px'}}>
              {QUESTION_STATUS_LABEL[v.status as keyof typeof QUESTION_STATUS_LABEL]}
            </span>
          </div>
        </div>
      ))}
    </div>
  )
}
