import { useMemo } from 'react'
import { useAppStore } from '../store/useAppStore'

export default function SummaryPanel() {
  const data = useAppStore((s) => s.data)

  const summaries = useMemo(() => {
    if (!data) return []
    const tagMap: Record<string, { count: number; examples: string[] }> = {}
    data.notes.forEach((n) => {
      if (n.tags.length === 0) {
        if (!tagMap['未分类']) tagMap['未分类'] = { count: 0, examples: [] }
        tagMap['未分类'].count++
        tagMap['未分类'].examples.push(n.text.slice(0, 30))
      }
      n.tags.forEach((t) => {
        if (!tagMap[t]) tagMap[t] = { count: 0, examples: [] }
        tagMap[t].count++
        tagMap[t].examples.push(n.text.slice(0, 30))
      })
    })
    return Object.entries(tagMap)
      .map(([tag, info]) => ({
        title: `${tag} 相关笔记`,
        content: info.examples.slice(0, 3).join(' · '),
        tag,
        count: info.count,
      }))
      .sort((a, b) => b.count - a.count)
  }, [data])

  return (
    <div className="pn act">
      <div className="st">智能总结</div>
      {summaries.length === 0 ? (
        <div className="em"><i className="fas fa-layer-group" style={{fontSize:'2rem',color:'var(--text-placeholder)'}}></i><div>暂无笔记可总结</div></div>
      ) : (
        <div className="sg">
          {summaries.map((s) => (
            <div className="sc" key={s.tag}>
              <div style={{display:'flex',justifyContent:'space-between',alignItems:'center',marginBottom:'8px'}}>
                <span className="qtg">#{s.tag}</span>
              </div>
              <div style={{fontSize:'.8rem'}}>
                <strong>{s.title}</strong><br />{s.content}
              </div>
              <div style={{fontSize:'.65rem',color:'var(--text-muted)'}}>📄 {s.count} 条</div>
            </div>
          ))}
        </div>
      )}
    </div>
  )
}
