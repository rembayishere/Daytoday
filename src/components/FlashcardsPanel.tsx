import { useState } from 'react'
import { useAppStore } from '../store/useAppStore'

function FlashcardItem({ f, activeDateFilter }: { f: import("../types").Flashcard; activeDateFilter: string | null }) {
  const [editing, setEditing] = useState(false)
  const [front, setFront] = useState(f.front)
  const [back, setBack] = useState(f.back)
  const [tag, setTag] = useState(f.tag)
  const [flipped, setFlipped] = useState(false)

  const handleSave = async () => {
    if (!front.trim() || !back.trim()) return
    await useAppStore.getState().updateFlashcard(f.id, front.trim(), back.trim(), tag.trim() || '未分类')
    setEditing(false)
  }

  const handleDelete = async () => {
    try {
      await useAppStore.getState().deleteFlashcard(f.id)
    } catch (e) {
      console.error('删除闪卡失败', e)
    }
  }

  const learningResult = useAppStore((s) => s.learning.results[f.id])

  if (editing) {
    return (
      <div className="flashcard-item" style={{cursor:'default'}}>
        <input value={front} onChange={(e) => setFront(e.target.value)} placeholder="正面" style={{width:'100%',padding:'6px 10px',border:'1px solid var(--border-input)',borderRadius:'10px',fontSize:'.78rem',marginBottom:'6px',outline:'none',background:'var(--bg-input)',color:'var(--text-primary)'}} />
        <input value={back} onChange={(e) => setBack(e.target.value)} placeholder="背面" style={{width:'100%',padding:'6px 10px',border:'1px solid var(--border-input)',borderRadius:'10px',fontSize:'.78rem',marginBottom:'6px',outline:'none',background:'var(--bg-input)',color:'var(--text-primary)'}} />
        <input value={tag} onChange={(e) => setTag(e.target.value)} placeholder="标签" style={{width:'100%',padding:'6px 10px',border:'1px solid var(--border-input)',borderRadius:'10px',fontSize:'.78rem',marginBottom:'6px',outline:'none',background:'var(--bg-input)',color:'var(--text-primary)'}} />
        <div style={{display:'flex',gap:'6px',marginTop:'4px'}}>
          <button className="btn btn-primary" style={{padding:'4px 14px',fontSize:'.72rem'}} onClick={handleSave}>保存</button>
          <button className="btn btn-secondary" style={{padding:'4px 14px',fontSize:'.72rem'}} onClick={() => setEditing(false)}>取消</button>
        </div>
      </div>
    )
  }

  return (
    <div className={`flashcard-item${activeDateFilter && f.date === activeDateFilter ? ' filtered-highlight' : ''}`}>
      <div className="fc-front">
        <span
          className={`flip-indicator${flipped ? ' flipped' : ''}`}
          onClick={() => setFlipped(!flipped)}
          title={flipped ? '隐藏答案' : '显示答案'}
        >
          <i className={`fas fa-chevron-${flipped ? 'up' : 'down'}`}></i>
        </span>
        {f.front}
        {learningResult && (
          <span style={{marginLeft:'8px',fontSize:'.68rem',color: learningResult === 'remembered' ? '#22c55e' : learningResult === 'hesitated' ? '#f59e0b' : '#ef4444'}}>
            <i className={`fas fa-${learningResult === 'remembered' ? 'check-circle' : learningResult === 'hesitated' ? 'question-circle' : 'times-circle'}`}></i>
          </span>
        )}
      </div>
      <div className={`fc-back${flipped ? ' visible' : ''}`}>{f.back}</div>
      <div className="fc-meta">
        <span className="fc-tag">#{f.tag}</span>
        <span className="fc-date"><i className="far fa-clock"></i> {f.date}</span>
      </div>
      <div className="fc-actions">
        <button onClick={(e) => { e.stopPropagation(); setFront(f.front); setBack(f.back); setTag(f.tag); setEditing(true) }} title="编辑"><i className="fas fa-pen"></i></button>
        <button onClick={(e) => { e.stopPropagation(); handleDelete() }} title="删除"><i className="fas fa-trash"></i></button>
      </div>
    </div>
  )
}

export default function FlashcardsPanel() {
  const data = useAppStore((s) => s.data)
  const activeDateFilter = useAppStore((s) => s.activeDateFilter)
  const startLearning = useAppStore((s) => s.startLearning)
  const learningActive = useAppStore((s) => s.learning.active)
  const [front, setFront] = useState('')
  const [back, setBack] = useState('')
  const [tag, setTag] = useState('')

  const handleAdd = async () => {
    if (!front.trim() || !back.trim()) return
    await useAppStore.getState().addFlashcard(front.trim(), back.trim(), tag.trim() || '未分类')
    setFront('')
    setBack('')
    setTag('')
  }

  const flashcards = data
    ? data.flashcards.filter((f) => !activeDateFilter || f.date === activeDateFilter)
    : []

  const handleStartStudy = () => {
    if (flashcards.length === 0) return
    startLearning(flashcards)
  }

  return (
    <div className="pn act">
      <div style={{display:'flex',alignItems:'center',justifyContent:'space-between',flexShrink:0}}>
        <div className="st">闪记卡片</div>
        {flashcards.length > 0 && !learningActive && (
          <button className="study-btn" onClick={handleStartStudy}>
            <i className="fas fa-graduation-cap"></i> 学习模式
          </button>
        )}
      </div>
      <div className="flashcard-add-row" style={{flexShrink:0}}>
        <input value={front} onChange={(e) => setFront(e.target.value)} placeholder="正面问题" style={{padding:'10px 12px',border:'1px solid var(--border-input)',borderRadius:'10px',fontSize:'.88rem',outline:'none',background:'var(--bg-input)',color:'var(--text-primary)'}} />
        <input value={back} onChange={(e) => setBack(e.target.value)} placeholder="背面答案" style={{padding:'10px 12px',border:'1px solid var(--border-input)',borderRadius:'10px',fontSize:'.88rem',outline:'none',background:'var(--bg-input)',color:'var(--text-primary)'}} />
        <input value={tag} onChange={(e) => setTag(e.target.value)} placeholder="标签（可选）" style={{padding:'10px 12px',border:'1px solid var(--border-input)',borderRadius:'10px',fontSize:'.88rem',outline:'none',background:'var(--bg-input)',color:'var(--text-primary)'}} />
        <button className="btn btn-primary" onClick={handleAdd} style={{padding:'10px 20px',fontSize:'.88rem'}}>添加</button>
      </div>
      <div className="fg">
        {flashcards.length === 0 ? (
          <div className="em">{activeDateFilter ? '该日期无闪记卡片' : '暂无闪记卡片，添加一张吧'}</div>
        ) : (
          flashcards.map((f) => <FlashcardItem key={f.id} f={f} activeDateFilter={activeDateFilter} />)
        )}
      </div>
    </div>
  )
}
