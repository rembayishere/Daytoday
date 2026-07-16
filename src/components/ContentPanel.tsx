import { useAppStore } from '../store/useAppStore'
import NotesPanel from './NotesPanel'
import SummaryPanel from './SummaryPanel'
import ClipsPanel from './ClipsPanel'
import FilesPanel from './FilesPanel'
import QuestionsPanel from './QuestionsPanel'
import FlashcardsPanel from './FlashcardsPanel'
import TaskBoard from './TaskBoard'
import SettingsPanel from './SettingsPanel'

const PANELS: Record<string, React.FC> = {
  notes: NotesPanel,
  summary: SummaryPanel,
  clips: ClipsPanel,
  files: FilesPanel,
  questions: QuestionsPanel,
  flashcards: FlashcardsPanel,
  tasks: TaskBoard,
  settings: SettingsPanel,
}

export default function ContentPanel() {
  const activePanel = useAppStore((s) => s.activePanel)
  const activeDateFilter = useAppStore((s) => s.activeDateFilter)
  const setActiveDateFilter = useAppStore((s) => s.setActiveDateFilter)
  const data = useAppStore((s) => s.data)

  const content = data && activeDateFilter
    ? Object.values(data).reduce((acc: string[], item) => {
        if (Array.isArray(item)) {
          item.forEach((i: any) => {
            if (i.date === activeDateFilter) {
              if (i.text) acc.push('📝' + i.text.slice(0,20))
              else if (i.question) acc.push('❓' + i.question)
              else if (i.front) acc.push('🃏' + i.front)
            }
          })
        }
        return acc
      }, [])
    : []

  const Panel = PANELS[activePanel]

  return (
    <div className="content-panel">
      <div className="top-bar">
        <div className="sbox">
          <i className="fas fa-search"></i>
          <input type="text" placeholder="搜索..." />
        </div>
        {activeDateFilter && (
          <div className="jump-info show">
            <i className="fas fa-filter"></i>
            <span>📅 {activeDateFilter}</span>
            <span className="filter-summary">{content.length ? content.join(' ') : '无记录'}</span>
            <span className="clear-filter" onClick={() => setActiveDateFilter(null)}>✕</span>
          </div>
        )}
      </div>
      <div className="pc">
        {Panel && <Panel />}
      </div>
    </div>
  )
}
