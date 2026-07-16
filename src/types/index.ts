export interface Note {
  id: number
  text: string
  tags: string[]
  time: string
  date: string
}

export interface Summary {
  title: string
  content: string
  tag: string
  count: number
}

export interface Clip {
  id: number
  title: string
  url: string
}

export type QuestionStatus = 'open' | 'in-progress' | 'answered'

export interface Version {
  id: number
  q_id: number
  timestamp: string
  question: string
  status: QuestionStatus
  desc: string
  note: string
  tags: string[]
  change_desc: string
}

export interface Subtask {
  id: number
  text: string
  done: boolean
}

export interface Question {
  id: number
  question: string
  desc: string
  note: string
  status: QuestionStatus
  tags: string[]
  created: string
  date: string
  versions: Version[]
  filtered?: boolean
}

export interface Flashcard {
  id: number
  front: string
  back: string
  tag: string
  date: string
}

export type TaskPriority = 'high' | 'medium' | 'low'

export interface Task {
  id: number
  title: string
  priority: TaskPriority
  date: string
  note?: string
  subtasks?: Subtask[]
}

export interface TaskBoard {
  todo: Task[]
  doing: Task[]
  done: Task[]
}

export interface AiConfig {
  url: string
  model: string
  key: string
}

export interface WebdavConfig {
  url: string
  user: string
  pass: string
  path: string
  encrypt: boolean
  enc_pass: string
  sync_notes?: boolean
  sync_summaries?: boolean
  sync_clips?: boolean
  sync_questions?: boolean
  sync_flashcards?: boolean
  sync_tasks?: boolean
  sync_attachments?: boolean
  sync_mode?: string
  sync_interval?: number
  pull_mode?: string
  settings_pass?: string
  sync_settings?: boolean
}

export interface ShortcutConfig {
  send_note: string
  quick_note: string
}

export interface Attachment {
  id: number
  filename: string
  size: number
  note_ids: number[]
  folder: string
  created: string
  date: string
}

export interface RemoteAttachment {
  filename: string
  size: number
  exists_local: boolean
}

export interface BootstrapConfig {
  data_dir: string
}

export interface DataDirResult {
  old_dir: string
  new_dir: string
}

export interface AppData {
  notes: Note[]
  summaries: Summary[]
  clips: Clip[]
  questions: Question[]
  flashcards: Flashcard[]
  attachments: Attachment[]
  tasks: TaskBoard
  next_task_id: number
  next_fc_id: number
  ai_config: AiConfig
  webdav_config: WebdavConfig
  shortcuts: ShortcutConfig
  attachment_dir?: string
  data_dir?: string
}

export const QUESTION_STATUS_LABEL: Record<QuestionStatus, string> = {
  'open': '待解决',
  'in-progress': '进行中',
  'answered': '已解答',
}
