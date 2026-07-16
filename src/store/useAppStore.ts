import { create } from 'zustand'
import { invoke } from '@tauri-apps/api/core'
import type { AppData, DataDirResult, Flashcard, RemoteAttachment, AttachmentMigrateResult } from '../types'

interface LearningState {
  active: boolean
  queue: Flashcard[]
  index: number
  results: Record<number, 'remembered' | 'hesitated' | 'forgotten'>
}

interface AppStore {
  data: AppData | null
  activePanel: string
  activeDateFilter: string | null
  currentMonth: number
  currentYear: number
  editingTask: { id: number | null; status: string | null }
  editingQuestionId: number | null
  theme: 'blue' | 'indigo'
  learning: LearningState
  loading: boolean
  error: string | null

  setActivePanel: (panel: string) => void
  setActiveDateFilter: (date: string | null) => void
  setEditingTask: (id: number | null, status: string | null) => void
  setEditingQuestion: (id: number | null) => void
  setTheme: (theme: 'blue' | 'indigo') => void
  startLearning: (cards: Flashcard[]) => void
  stopLearning: () => void
  nextLearningCard: () => void
  assessLearningCard: (result: 'remembered' | 'hesitated' | 'forgotten') => void

  loadData: () => Promise<void>
  updateNote: (id: number, text: string) => Promise<void>
  deleteNote: (id: number) => Promise<void>
  addClip: (title: string, url: string) => Promise<void>
  updateClip: (id: number, title: string, url: string) => Promise<void>
  deleteClip: (id: number) => Promise<void>
  addNote: (text: string) => Promise<void>
  addQuestion: (question: string, desc: string, tags: string[]) => Promise<void>
  updateQuestion: (id: number, question: string, desc: string, tags: string[]) => Promise<void>
  cycleQuestion: (id: number) => Promise<void>
  deleteQuestion: (id: number) => Promise<void>
  updateQuestionNote: (id: number, note: string) => Promise<void>
  addFlashcard: (front: string, back: string, tag: string) => Promise<void>
  updateFlashcard: (id: number, front: string, back: string, tag: string) => Promise<void>
  deleteFlashcard: (id: number) => Promise<void>
  addTask: (title: string, status: string) => Promise<void>
  updateTask: (id: number, status: string, title: string, priority: string, date: string, note: string) => Promise<void>
  deleteTask: (id: number, status: string) => Promise<void>
  moveTask: (id: number, from: string, to: string) => Promise<void>
  addTaskSubtask: (taskId: number, status: string, text: string) => Promise<void>
  toggleTaskSubtask: (taskId: number, status: string, subtaskId: number) => Promise<void>
  deleteTaskSubtask: (taskId: number, status: string, subtaskId: number) => Promise<void>
  addAttachment: (name: string, data: number[]) => Promise<void>
  deleteAttachment: (id: number) => Promise<void>
  linkNoteAttachment: (attachmentId: number, noteId: number) => Promise<void>
  unlinkNoteAttachment: (attachmentId: number, noteId: number) => Promise<void>
  moveAttachment: (id: number, folder: string) => Promise<void>
  openAttachmentFolder: (id: number) => Promise<void>
  listRemoteAttachments: () => Promise<RemoteAttachment[]>
  saveAttachmentDir: (dir: string) => Promise<AttachmentMigrateResult>
  deleteAttachmentDirBackup: (path: string) => Promise<void>
  saveAiConfig: (url: string, model: string, key: string) => Promise<void>
  saveWebdavConfig: (
    url: string, user: string, pass: string, path: string,
    encrypt: boolean, enc_pass: string,
    sync_notes: boolean, sync_summaries: boolean, sync_clips: boolean,
    sync_questions: boolean, sync_flashcards: boolean, sync_tasks: boolean, sync_attachments: boolean,
    sync_mode: string,
    sync_interval: number, pull_mode: string, settings_pass: string, sync_settings: boolean,
  ) => Promise<void>
  syncPull: () => Promise<void>
  saveShortcuts: (send_note: string, quick_note: string) => Promise<void>
  saveDataDir: (dir: string) => Promise<DataDirResult>
  deleteFile: (path: string) => Promise<void>
  openFileExplorer: (path: string) => Promise<void>

  prevMonth: () => void
  nextMonth: () => void
  setMonth: (year: number, month: number) => void
}

function getInitialTheme(): 'blue' | 'indigo' {
  try {
    const stored = localStorage.getItem('flomo_plus_theme')
    if (stored === 'blue' || stored === 'indigo') return stored
  } catch {}
  return 'blue'
}

export const useAppStore = create<AppStore>((set) => ({
  data: null,
  activePanel: 'notes',
  activeDateFilter: null,
  currentMonth: new Date().getMonth(),
  currentYear: new Date().getFullYear(),
  editingTask: { id: null, status: null },
  editingQuestionId: null,
  theme: getInitialTheme(),
  learning: { active: false, queue: [], index: 0, results: {} },
  loading: false,
  error: null,

  setActivePanel: (panel) => set({ activePanel: panel }),
  setActiveDateFilter: (date) => set({ activeDateFilter: date }),
  setEditingTask: (id, status) => set({ editingTask: { id, status } }),
  setEditingQuestion: (id) => set({ editingQuestionId: id }),
  setTheme: (theme) => {
    try { localStorage.setItem('flomo_plus_theme', theme) } catch {}
    document.documentElement.setAttribute('data-theme', theme)
    set({ theme })
  },
  startLearning: (cards) => set({ learning: { active: true, queue: [...cards], index: 0, results: {} } }),
  stopLearning: () => set((s) => ({ learning: { ...s.learning, active: false } })),
  nextLearningCard: () => set((s) => ({ learning: { ...s.learning, index: Math.min(s.learning.index + 1, s.learning.queue.length) } })),
  assessLearningCard: (result) => set((s) => {
    const card = s.learning.queue[s.learning.index]
    if (!card) return s
    return { learning: { ...s.learning, results: { ...s.learning.results, [card.id]: result } } }
  }),

  loadData: async () => {
    set({ loading: true, error: null })
    try {
      const data = await invoke<AppData>('get_all_data')
      set({ data, loading: false })
    } catch (e) { set({ loading: false, error: String(e) }); console.error('loadData failed:', e) }
  },

  addNote: async (text) => {
    try {
      const data = await invoke<AppData>('add_note', { text })
      set({ data })
    } catch (e) { console.error('addNote failed:', e) }
  },

  updateNote: async (id, text) => {
    try {
      const data = await invoke<AppData>('update_note', { id, text })
      set({ data })
    } catch (e) { console.error('updateNote failed:', e) }
  },

  deleteNote: async (id) => {
    try {
      const data = await invoke<AppData>('delete_note', { id })
      set({ data })
    } catch (e) { console.error('deleteNote failed:', e) }
  },

  addClip: async (title, url) => {
    try {
      const data = await invoke<AppData>('add_clip', { title, url })
      set({ data })
    } catch (e) { console.error('addClip failed:', e) }
  },

  updateClip: async (id, title, url) => {
    try {
      const data = await invoke<AppData>('update_clip', { id, title, url })
      set({ data })
    } catch (e) { console.error('updateClip failed:', e) }
  },

  deleteClip: async (id) => {
    try {
      const data = await invoke<AppData>('delete_clip', { id })
      set({ data })
    } catch (e) { console.error('deleteClip failed:', e) }
  },

  addQuestion: async (question, desc, tags) => {
    try {
      const data = await invoke<AppData>('add_question', { question, desc, tags })
      set({ data })
    } catch (e) { console.error('addQuestion failed:', e) }
  },

  updateQuestion: async (id, question, desc, tags) => {
    try {
      const data = await invoke<AppData>('update_question', { id, question, desc, tags })
      set({ data })
    } catch (e) { console.error('updateQuestion failed:', e) }
  },

  cycleQuestion: async (id) => {
    try {
      const data = await invoke<AppData>('cycle_question', { id })
      set({ data })
    } catch (e) { console.error('cycleQuestion failed:', e) }
  },

  deleteQuestion: async (id) => {
    try {
      const data = await invoke<AppData>('delete_question', { id })
      set({ data })
    } catch (e) { console.error('deleteQuestion failed:', e) }
  },

  updateQuestionNote: async (id, note) => {
    try {
      const data = await invoke<AppData>('update_question_note', { id, note })
      set({ data })
    } catch (e) { console.error('updateQuestionNote failed:', e) }
  },

  addFlashcard: async (front, back, tag) => {
    try {
      const data = await invoke<AppData>('add_flashcard', { front, back, tag })
      set({ data })
    } catch (e) { console.error('addFlashcard failed:', e) }
  },

  updateFlashcard: async (id, front, back, tag) => {
    try {
      const data = await invoke<AppData>('update_flashcard', { id, front, back, tag })
      set({ data })
    } catch (e) { console.error('updateFlashcard failed:', e) }
  },

  deleteFlashcard: async (id) => {
    try {
      const data = await invoke<AppData>('delete_flashcard', { id })
      set({ data })
    } catch (e) { console.error('deleteFlashcard failed:', e) }
  },

  addTask: async (title, status) => {
    try {
      const data = await invoke<AppData>('add_task', { title, status })
      set({ data })
    } catch (e) { console.error('addTask failed:', e) }
  },

  updateTask: async (id, status, title, priority, date, note) => {
    try {
      const data = await invoke<AppData>('update_task', { id, status, title, priority, date, note })
      set({ data })
    } catch (e) { console.error('updateTask failed:', e) }
  },

  deleteTask: async (id, status) => {
    try {
      const data = await invoke<AppData>('delete_task', { id, status })
      set({ data })
    } catch (e) { console.error('deleteTask failed:', e) }
  },

  moveTask: async (id, from, to) => {
    try {
      const data = await invoke<AppData>('move_task', { id, from, to })
      set({ data })
    } catch (e) { console.error('moveTask failed:', e) }
  },

  addTaskSubtask: async (taskId, status, text) => {
    try {
      const data = await invoke<AppData>('add_task_subtask', { taskId, status, text })
      set({ data })
    } catch (e) { console.error('addTaskSubtask failed:', e) }
  },

  toggleTaskSubtask: async (taskId, status, subtaskId) => {
    try {
      const data = await invoke<AppData>('toggle_task_subtask', { taskId, status, subtaskId })
      set({ data })
    } catch (e) { console.error('toggleTaskSubtask failed:', e) }
  },

  deleteTaskSubtask: async (taskId, status, subtaskId) => {
    try {
      const data = await invoke<AppData>('delete_task_subtask', { taskId, status, subtaskId })
      set({ data })
    } catch (e) { console.error('deleteTaskSubtask failed:', e) }
  },

  addAttachment: async (name, data) => {
    try {
      const result = await invoke<AppData>('add_attachment', { name, data })
      set({ data: result })
    } catch (e) { console.error('addAttachment failed:', e) }
  },
  deleteAttachment: async (id) => {
    try {
      const result = await invoke<AppData>('delete_attachment', { id })
      set({ data: result })
    } catch (e) { console.error('deleteAttachment failed:', e) }
  },
  linkNoteAttachment: async (attachmentId, noteId) => {
    try {
      const result = await invoke<AppData>('link_note_attachment', { attachmentId, noteId })
      set({ data: result })
    } catch (e) { console.error('linkNoteAttachment failed:', e) }
  },
  unlinkNoteAttachment: async (attachmentId, noteId) => {
    try {
      const result = await invoke<AppData>('unlink_note_attachment', { attachmentId, noteId })
      set({ data: result })
    } catch (e) { console.error('unlinkNoteAttachment failed:', e) }
  },
  moveAttachment: async (id, folder) => {
    try {
      const result = await invoke<AppData>('move_attachment', { id, folder })
      set({ data: result })
    } catch (e) { console.error('moveAttachment failed:', e) }
  },
  openAttachmentFolder: async (id) => {
    try {
      await invoke('open_attachment_folder', { id })
    } catch (e) { console.error('openAttachmentFolder failed:', e) }
  },
  listRemoteAttachments: async () => {
    try {
      return await invoke<RemoteAttachment[]>('list_remote_attachments')
    } catch (e) {
      console.error('listRemoteAttachments failed:', e)
      throw e
    }
  },
  saveAttachmentDir: async (dir) => {
    try {
      const result = await invoke<AttachmentMigrateResult>('save_attachment_dir', { dir })
      return result
    } catch (e) { console.error('saveAttachmentDir failed:', e); throw e }
  },
  deleteAttachmentDirBackup: async (path) => {
    try {
      await invoke('delete_file', { path })
    } catch (e) { console.error('deleteAttachmentDirBackup failed:', e) }
  },
  saveAiConfig: async (url, model, key) => {
    try {
      const data = await invoke<AppData>('save_ai_config', { url, model, key })
      set({ data })
    } catch (e) { console.error('saveAiConfig failed:', e) }
  },

  saveWebdavConfig: async (url, user, pass, path, encrypt, enc_pass, sync_notes, sync_summaries, sync_clips, sync_questions, sync_flashcards, sync_tasks, sync_attachments, sync_mode, sync_interval, pull_mode, settings_pass, sync_settings) => {
    try {
      const data = await invoke<AppData>('save_webdav_config', {
        url, user, pass, path, encrypt, encPass: enc_pass,
        syncNotes: sync_notes, syncSummaries: sync_summaries, syncClips: sync_clips,
        syncQuestions: sync_questions, syncFlashcards: sync_flashcards, syncTasks: sync_tasks, syncAttachments: sync_attachments,
        syncMode: sync_mode, syncInterval: sync_interval,
        pullMode: pull_mode, settingsPass: settings_pass, syncSettings: sync_settings,
      })
      set({ data })
    } catch (e) { console.error('saveWebdavConfig failed:', e) }
  },
  syncPull: async () => {
    try {
      await invoke('sync_pull')
      const data = await invoke<AppData>('get_all_data')
      set({ data })
    } catch (e) { console.error('syncPull failed:', e) }
  },

  saveShortcuts: async (send_note, quick_note) => {
    try {
      const data = await invoke<AppData>('save_shortcuts', { sendNote: send_note, quickNote: quick_note })
      set({ data })
    } catch (e) { console.error('saveShortcuts failed:', e) }
  },

  saveDataDir: async (dir) => {
    try {
      const result = await invoke<DataDirResult>('save_data_dir', { dir })
      return result
    } catch (e) { console.error('saveDataDir failed:', e); throw e }
  },

  deleteFile: async (path) => {
    try {
      await invoke('delete_file', { path })
    } catch (e) { console.error('deleteFile failed:', e) }
  },

  openFileExplorer: async (path) => {
    try {
      await invoke('open_file_explorer', { path })
    } catch (e) { console.error('openFileExplorer failed:', e) }
  },

  prevMonth: () => set((s) => {
    let m = s.currentMonth - 1
    let y = s.currentYear
    if (m < 0) { m = 11; y-- }
    return { currentMonth: m, currentYear: y }
  }),

  nextMonth: () => set((s) => {
    const now = new Date()
    if (s.currentYear === now.getFullYear() && s.currentMonth >= now.getMonth()) return s
    let m = s.currentMonth + 1
    let y = s.currentYear
    if (m > 11) { m = 0; y++ }
    return { currentMonth: m, currentYear: y }
  }),

  setMonth: (year, month) => set({ currentYear: year, currentMonth: month, activeDateFilter: null }),
}))
