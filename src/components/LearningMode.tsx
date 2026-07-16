import { useState, useEffect, useCallback } from 'react'
import { useAppStore } from '../store/useAppStore'

export default function LearningMode() {
  const learning = useAppStore((s) => s.learning)
  const stopLearning = useAppStore((s) => s.stopLearning)
  const nextLearningCard = useAppStore((s) => s.nextLearningCard)
  const assessLearningCard = useAppStore((s) => s.assessLearningCard)
  const [flipped, setFlipped] = useState(false)

  const card = learning.queue[learning.index]
  const total = learning.queue.length
  const doneCount = Object.keys(learning.results).length
  const isDone = learning.index >= total

  const handleFlip = useCallback(() => {
    if (!flipped) setFlipped(true)
  }, [flipped])

  const handleAssess = useCallback((result: 'remembered' | 'hesitated' | 'forgotten') => {
    assessLearningCard(result)
    setFlipped(false)
    setTimeout(() => nextLearningCard(), 200)
  }, [assessLearningCard, nextLearningCard])

  useEffect(() => {
    const handleKey = (e: KeyboardEvent) => {
      if (!learning.active) return
      if (e.key === 'Escape') { stopLearning(); return }
      if (e.key === ' ' || e.key === 'Enter') {
        e.preventDefault()
        if (!flipped) { handleFlip(); return }
      }
      if (flipped) {
        if (e.key === '1' || e.key === 'r') handleAssess('remembered')
        if (e.key === '2' || e.key === 'h') handleAssess('hesitated')
        if (e.key === '3' || e.key === 'f') handleAssess('forgotten')
      }
    }
    window.addEventListener('keydown', handleKey)
    return () => window.removeEventListener('keydown', handleKey)
  }, [learning.active, flipped, handleFlip, handleAssess, stopLearning])

  if (!learning.active) return null

  return (
    <>
      <div className="learning-header">
        <div className="learning-progress">
          {isDone ? '完成' : `第 ${learning.index + 1} / ${total} 张`}
          {doneCount > 0 && ` · 已评 ${doneCount} 张`}
        </div>
        <button className="learning-exit" onClick={stopLearning}>
          <i className="fas fa-times"></i> 退出学习
        </button>
      </div>
      <div className="learning-overlay" onClick={(e) => { if (e.target === e.currentTarget) stopLearning() }}>
        {isDone ? (
          <div className="learning-done">
            <h2><i className="fas fa-trophy"></i> 学习完成！</h2>
            <p>
              记得: {Object.values(learning.results).filter(r => r === 'remembered').length} ·
              模糊: {Object.values(learning.results).filter(r => r === 'hesitated').length} ·
              忘记: {Object.values(learning.results).filter(r => r === 'forgotten').length}
            </p>
            <button className="btn" onClick={stopLearning}>返回</button>
          </div>
        ) : card ? (
          <div className={`learning-card${flipped ? ' flipped' : ''}`} onClick={handleFlip}>
            <div className="learning-card-inner">
              <div className="learning-front">
                <div className="learning-label">问题</div>
                <div className="learning-content">{card.front}</div>
                <div className="learning-hint">
                  <i className="fas fa-mouse-pointer"></i> 点击卡片或按 <kbd style={{background:'rgba(0,0,0,0.1)',padding:'2px 6px',borderRadius:'4px',fontSize:'.7rem',fontFamily:'monospace'}}>Space</kbd> 翻转
                </div>
              </div>
              <div className="learning-back">
                <div className="learning-label">答案</div>
                <div className="learning-content">{card.back}</div>
                <div className="learning-actions" onClick={(e) => e.stopPropagation()}>
                  <button className="btn-remember" onClick={() => handleAssess('remembered')}>
                    <i className="fas fa-check"></i> 记得 (1)
                  </button>
                  <button className="btn-hesitate" onClick={() => handleAssess('hesitated')}>
                    <i className="fas fa-question"></i> 模糊 (2)
                  </button>
                  <button className="btn-forgot" onClick={() => handleAssess('forgotten')}>
                    <i className="fas fa-times"></i> 忘记 (3)
                  </button>
                </div>
              </div>
            </div>
          </div>
        ) : null}
      </div>
    </>
  )
}
