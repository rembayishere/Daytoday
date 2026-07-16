import { useMemo, useState, useRef, useEffect } from 'react'
import { useAppStore } from '../store/useAppStore'

function getMonthWeeks(year: number, month: number): Date[][] {
  const firstDay = new Date(year, month, 1)
  const lastDay = new Date(year, month + 1, 0)
  const dayOfWeek = firstDay.getDay()
  const start = new Date(firstDay)
  const offset = dayOfWeek === 0 ? 6 : dayOfWeek - 1
  start.setDate(start.getDate() - offset)
  const endDayOfWeek = lastDay.getDay()
  const endOffset = endDayOfWeek === 0 ? 0 : 7 - endDayOfWeek
  lastDay.setDate(lastDay.getDate() + endOffset)
  const weeks: Date[][] = []
  const current = new Date(start)
  while (current <= lastDay) {
    const week: Date[] = []
    for (let i = 0; i < 7; i++) {
      const day = new Date(current)
      day.setDate(day.getDate() + i)
      week.push(day)
    }
    weeks.push(week)
    current.setDate(current.getDate() + 7)
  }
  return weeks
}

function dateKey(d: Date): string {
  const y = d.getFullYear()
  const m = String(d.getMonth() + 1).padStart(2, '0')
  const day = String(d.getDate()).padStart(2, '0')
  return `${y}-${m}-${day}`
}

function getLevel(count: number): number {
  if (count === 0) return 0
  if (count <= 2) return 1
  if (count <= 5) return 2
  if (count <= 8) return 3
  return 4
}

export default function Heatmap() {
  const data = useAppStore((s) => s.data)
  const activeDateFilter = useAppStore((s) => s.activeDateFilter)
  const setActiveDateFilter = useAppStore((s) => s.setActiveDateFilter)
  const currentYear = useAppStore((s) => s.currentYear)
  const currentMonth = useAppStore((s) => s.currentMonth)

  const [showMonthPicker, setShowMonthPicker] = useState(false)
  const monthPickerRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (monthPickerRef.current && !monthPickerRef.current.contains(e.target as Node)) {
        setShowMonthPicker(false)
      }
    }
    if (showMonthPicker) document.addEventListener('mousedown', handleClickOutside)
    return () => document.removeEventListener('mousedown', handleClickOutside)
  }, [showMonthPicker])

  const stats = useMemo(() => {
    if (!data) return { counts: {}, total: 0 }
    const counts: Record<string, number> = {}
    data.notes.forEach((n) => { counts[n.date] = (counts[n.date] || 0) + 1 })
    data.questions.forEach((q) => { counts[q.date] = (counts[q.date] || 0) + 1 })
    data.flashcards.forEach((f) => { counts[f.date] = (counts[f.date] || 0) + 1 })
    Object.values(data.tasks).forEach((list) =>
      list.forEach((t: { date: string }) => { counts[t.date] = (counts[t.date] || 0) + 1 })
    )
    const total = Object.values(counts).reduce((a, b) => a + b, 0)
    return { counts, total }
  }, [data])

  const monthCount = useMemo(() => {
    const weeks = getMonthWeeks(currentYear, currentMonth)
    let count = 0
    weeks.forEach((week) =>
      week.forEach((d) => {
        if (d.getMonth() === currentMonth && d.getFullYear() === currentYear) {
          count += stats.counts[dateKey(d)] || 0
        }
      })
    )
    return count
  }, [currentYear, currentMonth, stats])

  const weeks = useMemo(() => getMonthWeeks(currentYear, currentMonth), [currentYear, currentMonth])
  const monthNames = ['1月','2月','3月','4月','5月','6月','7月','8月','9月','10月','11月','12月']
  const dayNames = ['一','二','三','四','五','六','日']

  const handlePrevMonth = () => useAppStore.getState().prevMonth()

  const handleNextMonth = () => useAppStore.getState().nextMonth()

  const handleCellClick = (key: string) => {
    if (activeDateFilter === key) {
      setActiveDateFilter(null)
    } else {
      setActiveDateFilter(key)
    }
  }

  const handleMonthNav = (y: number, m: number) => {
    const now = new Date()
    if (y > now.getFullYear() || (y === now.getFullYear() && m > now.getMonth())) return
    useAppStore.getState().setMonth(y, m)
    setShowMonthPicker(false)
  }

  const now = new Date()
  const years = Array.from({ length: now.getFullYear() - 2019 }, (_, i) => 2020 + i)

  return (
    <>
      <div className="heatmap-header">
        <div className="heatmap-title"><i className="fas fa-fire"></i><span>活跃度热力图</span></div>
        <div className="heatmap-total">{stats.total} 条</div>
      </div>
      <div className="heatmap-month-nav">
        <button onClick={handlePrevMonth}><i className="fas fa-chevron-left"></i></button>
        <span className="heatmap-month-label" style={{cursor:'pointer'}} onClick={() => setShowMonthPicker(!showMonthPicker)}>
          {currentYear}年{monthNames[currentMonth]}
        </span>
        <button onClick={handleNextMonth}><i className="fas fa-chevron-right"></i></button>
      </div>
      {showMonthPicker && (
        <div ref={monthPickerRef} style={{position:'relative',textAlign:'center',marginBottom:'4px'}}>
          <div style={{display:'inline-flex',gap:'6px',alignItems:'center',background:'var(--bg-card)',border:'1px solid var(--border)',borderRadius:'10px',padding:'6px 12px',boxShadow:'0 4px 16px rgba(0,0,0,0.1)'}}>
            <select value={currentYear} onChange={(e) => {
              const y = Number(e.target.value)
              const m = Math.min(currentMonth, y >= now.getFullYear() ? now.getMonth() : 11)
              handleMonthNav(y, m)
            }} style={{padding:'4px 8px',border:'1px solid var(--border-input)',borderRadius:'8px',fontSize:'.78rem',background:'var(--bg-input)',color:'var(--text-primary)',outline:'none'}}>
              {years.map((y) => (
                <option key={y} value={y}>{y}年</option>
              ))}
            </select>
            <select value={currentMonth} onChange={(e) => {
              const m = Number(e.target.value)
              if (currentYear >= now.getFullYear() && m > now.getMonth()) return
              handleMonthNav(currentYear, m)
            }} style={{padding:'4px 8px',border:'1px solid var(--border-input)',borderRadius:'8px',fontSize:'.78rem',background:'var(--bg-input)',color:'var(--text-primary)',outline:'none'}}>
              {monthNames.map((name, i) => {
                const disabled = currentYear >= now.getFullYear() && i > now.getMonth()
                return (
                  <option key={i} value={i} disabled={disabled}>{name}</option>
                )
              })}
            </select>
          </div>
        </div>
      )}
      <div className="heatmap-month-stats">本月 {monthCount} 条记录</div>
      <div className="heatmap-body">
        <div className="heatmap-day-labels">
          {dayNames.map((d) => (<span key={d}>{d}</span>))}
        </div>
        <div className="heatmap-grid">
          {weeks.map((week, wi) => (
            <div className="heatmap-col" key={wi}>
              {week.map((d) => {
                const key = dateKey(d)
                const count = stats.counts[key] || 0
                const level = getLevel(count)
                const isCurrent = d.getMonth() === currentMonth && d.getFullYear() === currentYear
                const clicked = activeDateFilter === key
                return (
                  <div
                    key={key}
                    className={`heatmap-cell lv${level}${clicked ? ' clicked' : ''}`}
                    style={{ opacity: isCurrent ? 1 : 0.25 }}
                    data-date={key}
                    onClick={() => handleCellClick(key)}
                  >
                    <span className="heatmap-tooltip">
                      {d.getMonth()+1}月{d.getDate()}日 周{dayNames[d.getDay() === 0 ? 6 : d.getDay()-1]}<br />
                      总计 {count} 条
                    </span>
                  </div>
                )
              })}
            </div>
          ))}
        </div>
      </div>
      <div className="heatmap-legend">
        <span className="legend-label">少</span>
        <div className="legend-squares">
          <div style={{background:'var(--heat-lv0)'}}></div>
          <div style={{background:'var(--heat-lv1)'}}></div>
          <div style={{background:'var(--heat-lv2)'}}></div>
          <div style={{background:'var(--heat-lv3)'}}></div>
          <div style={{background:'var(--heat-lv4)'}}></div>
        </div>
        <span className="legend-label">多</span>
      </div>
    </>
  )
}
