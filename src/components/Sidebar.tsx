import { useAppStore } from '../store/useAppStore'
import Heatmap from './Heatmap'

const NAV_ITEMS = [
  { key: 'notes', icon: 'fa-pen', label: '笔记' },
  { key: 'summary', icon: 'fa-layer-group', label: '智能总结' },
  { key: 'clips', icon: 'fa-paperclip', label: '剪藏' },
  { key: 'files', icon: 'fa-folder-open', label: '资料' },
  { key: 'questions', icon: 'fa-question-circle', label: '问题追踪' },
  { key: 'flashcards', icon: 'fa-clone', label: '闪记卡片' },
  { key: 'tasks', icon: 'fa-tasks', label: '任务看板' },
]

export default function Sidebar() {
  const activePanel = useAppStore((s) => s.activePanel)
  const setActivePanel = useAppStore((s) => s.setActivePanel)

  return (
    <div className="sidebar">
      {NAV_ITEMS.map((item) => (
        <button
          key={item.key}
          className={`nav-item${activePanel === item.key ? ' active' : ''}`}
          onClick={() => setActivePanel(item.key)}
        >
          <i className={`fas ${item.icon}`}></i>
          <span>{item.label}</span>
        </button>
      ))}
      <div className="sidebar-spacer"></div>
      <button
        className={`nav-item${activePanel === 'settings' ? ' active' : ''}`}
        onClick={() => setActivePanel('settings')}
      >
        <i className="fas fa-cog"></i>
        <span>设置</span>
      </button>
      <div className="sidebar-heatmap-section">
        <Heatmap />
      </div>
    </div>
  )
}
