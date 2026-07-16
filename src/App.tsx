import { useEffect, useRef } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { useAppStore } from './store/useAppStore'
import Titlebar from './components/Titlebar'
import Sidebar from './components/Sidebar'
import ContentPanel from './components/ContentPanel'
import QuickNoteModal from './components/QuickNoteModal'
import TaskEditModal from './components/TaskEditModal'
import QuestionEditModal from './components/QuestionEditModal'
import LearningMode from './components/LearningMode'
import './App.css'

function App() {
  const loadData = useAppStore((s) => s.loadData)

  const data = useAppStore((s) => s.data)
  const syncTimerRef = useRef<number | null>(null)

  useEffect(() => {
    loadData()
  }, [loadData])

  useEffect(() => {
    const interval = data?.webdav_config?.sync_interval || 0
    if (syncTimerRef.current !== null) {
      clearInterval(syncTimerRef.current)
      syncTimerRef.current = null
    }
    if (interval > 0) {
      syncTimerRef.current = window.setInterval(async () => {
        try {
          await invoke('sync_webdav')
        } catch (e) {
          console.error('Auto sync failed:', e)
        }
      }, interval * 60 * 1000)
    }
    return () => {
      if (syncTimerRef.current !== null) {
        clearInterval(syncTimerRef.current)
      }
    }
  }, [data?.webdav_config?.sync_interval])

  return (
    <div className="demo-window">
      <Titlebar />
      <div className="main-layout">
        <Sidebar />
        <ContentPanel />
      </div>
      <button className="fab" id="fab"
        onClick={() => document.getElementById('quickModal')?.classList.add('op')}>
        <i className="fas fa-bolt"></i>
      </button>
      <QuickNoteModal />
      <TaskEditModal />
      <QuestionEditModal />
      <LearningMode />
    </div>
  )
}

export default App
