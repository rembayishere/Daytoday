import { useState, useEffect, useRef } from 'react'
import { useAppStore } from '../store/useAppStore'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { open } from '@tauri-apps/plugin-dialog'
import type { AttachmentMigrateResult } from '../types'

const SHORTCUT_KEYS: { key: 'send_note' | 'quick_note'; label: string }[] = [
  { key: 'send_note', label: '发送笔记（Ctrl+Enter）' },
  { key: 'quick_note', label: '快速记录弹窗（Alt+Q）' },
]

function formatShortcut(e: KeyboardEvent): string {
  if (e.key === 'Escape') return ''
  const parts: string[] = []
  if (e.ctrlKey) parts.push('Ctrl')
  if (e.altKey) parts.push('Alt')
  if (e.shiftKey) parts.push('Shift')
  if (e.metaKey) parts.push('Win')
  const special: Record<string, string> = { ' ': 'Space', 'Enter': 'Enter', 'Tab': 'Tab' }
  const key = special[e.key] || (e.key.length === 1 ? e.key.toUpperCase() : e.key)
  parts.push(key)
  return parts.join('+')
}

type SyncKey = 'sync_notes' | 'sync_summaries' | 'sync_clips' | 'sync_questions' | 'sync_flashcards' | 'sync_tasks' | 'sync_attachments'

const SYNC_ITEMS: { key: SyncKey; label: string }[] = [
  { key: 'sync_notes', label: '笔记' },
  { key: 'sync_summaries', label: '摘录总结' },
  { key: 'sync_clips', label: '剪藏' },
  { key: 'sync_questions', label: '问题追踪' },
  { key: 'sync_flashcards', label: '闪卡' },
  { key: 'sync_tasks', label: '任务看板' },
  { key: 'sync_attachments', label: '资料文件' },
]

export default function SettingsPanel() {
  const data = useAppStore((s) => s.data)
  const theme = useAppStore((s) => s.theme)
  const setTheme = useAppStore((s) => s.setTheme)
  const saveAiConfig = useAppStore((s) => s.saveAiConfig)
  const saveWebdavConfig = useAppStore((s) => s.saveWebdavConfig)
  const saveShortcuts = useAppStore((s) => s.saveShortcuts)
  const saveAttachmentDir = useAppStore((s) => s.saveAttachmentDir)
  const saveAttachmentMoveMode = useAppStore((s) => s.saveAttachmentMoveMode)
  const deleteAttachmentDirBackup = useAppStore((s) => s.deleteAttachmentDirBackup)
  const saveDataDir = useAppStore((s) => s.saveDataDir)
  const deleteFile = useAppStore((s) => s.deleteFile)
  const openFileExplorer = useAppStore((s) => s.openFileExplorer)
  const [aiUrl, setAiUrl] = useState('')
  const [aiModel, setAiModel] = useState('')
  const [aiKey, setAiKey] = useState('')
  const [aiResult, setAiResult] = useState('')
  const [modelList, setModelList] = useState<string[]>([])

  const [wdUrl, setWdUrl] = useState('')
  const [wdUser, setWdUser] = useState('')
  const [wdPass, setWdPass] = useState('')
  const [wdPath, setWdPath] = useState('/daytoday-backup/')
  const [wdEncPass, setWdEncPass] = useState('')
  const [wdEncAlgorithm, setWdEncAlgorithm] = useState('aes256-gcm')
  const [wdSyncNotes, setWdSyncNotes] = useState(true)
  const [wdSyncSummaries, setWdSyncSummaries] = useState(true)
  const [wdSyncClips, setWdSyncClips] = useState(true)
  const [wdSyncQuestions, setWdSyncQuestions] = useState(true)
  const [wdSyncFlashcards, setWdSyncFlashcards] = useState(true)
  const [wdSyncTasks, setWdSyncTasks] = useState(true)
  const [wdSyncAttachments, setWdSyncAttachments] = useState(true)
  const [wdAllowUnencrypted, setWdAllowUnencrypted] = useState(false)
  const [wdSyncMode, setWdSyncMode] = useState('upload')
  const [wdSyncInterval, setWdSyncInterval] = useState(0)
  const [wdPullMode, setWdPullMode] = useState('add')
  const [wdSyncSettings, setWdSyncSettings] = useState(true)
  const [wdSettingsPass, setWdSettingsPass] = useState('')
  const [wdResult, setWdResult] = useState('')
  const [scopeExpanded, setScopeExpanded] = useState(true)
  const [attDir, setAttDir] = useState('')
  const [attDirResult, setAttDirResult] = useState('')
  const [attMigrate, setAttMigrate] = useState<AttachmentMigrateResult | null>(null)
  const [attMoveMode, setAttMoveMode] = useState(false)

  const [dataDir, setDataDir] = useState('')
  const [oldDataDir, setOldDataDir] = useState('')
  const [dataDirResult, setDataDirResult] = useState('')

  const [shortcuts, setShortcuts] = useState({ send_note: 'Ctrl+Enter', quick_note: 'Alt+Q' })
  const [recording, setRecording] = useState<string | null>(null)
  const [pendingKeys, setPendingKeys] = useState('')
  const [scResult, setScResult] = useState('')
  const [syncProgress, setSyncProgress] = useState<number | null>(null)
  const [syncMessage, setSyncMessage] = useState('')

  const initDone = useRef(false)

  useEffect(() => {
    const unlisten = listen<{ progress: number; message: string }>('sync-progress', (event) => {
      setSyncProgress(event.payload.progress)
      setSyncMessage(event.payload.message)
      if (event.payload.progress >= 100) {
        setTimeout(() => { setSyncProgress(null); setSyncMessage('') }, 2500)
      }
    })
    return () => { unlisten.then((fn) => fn()) }
  }, [])

  useEffect(() => {
    if (data && !initDone.current) {
      initDone.current = true
      setAiUrl(data.ai_config.url || '')
      setAiModel(data.ai_config.model || '')
      setAiKey(data.ai_config.key || '')
      setWdUrl(data.webdav_config.url || '')
      setWdUser(data.webdav_config.user || '')
      setWdPass(data.webdav_config.pass || '')
      setWdPath(data.webdav_config.path || '/daytoday-backup/')
      setWdEncPass(data.webdav_config.enc_pass || '')
      setWdEncAlgorithm(data.webdav_config.enc_algorithm || 'aes256-gcm')
      setWdSyncNotes(data.webdav_config.sync_notes ?? true)
      setWdSyncSummaries(data.webdav_config.sync_summaries ?? true)
      setWdSyncClips(data.webdav_config.sync_clips ?? true)
      setWdSyncQuestions(data.webdav_config.sync_questions ?? true)
      setWdSyncFlashcards(data.webdav_config.sync_flashcards ?? true)
      setWdSyncTasks(data.webdav_config.sync_tasks ?? true)
      setWdSyncAttachments(data.webdav_config.sync_attachments ?? true)
      setWdAllowUnencrypted(data.webdav_config.allow_unencrypted_attachment ?? false)
      setWdSyncMode(data.webdav_config.sync_mode || 'upload')
      setWdSyncInterval(data.webdav_config.sync_interval || 0)
      setWdPullMode(data.webdav_config.pull_mode || 'add')
      setWdSyncSettings(data.webdav_config.sync_settings ?? true)
      setWdSettingsPass(data.webdav_config.settings_pass || '')
      setAttDir(data.attachment_dir || '')
      setAttMoveMode(data.attachment_move_mode ?? false)
      setDataDir(data.data_dir || '')
      setShortcuts({
        send_note: data.shortcuts?.send_note || 'Ctrl+Enter',
        quick_note: data.shortcuts?.quick_note || 'Alt+Q',
      })
    }
  }, [data])

  useEffect(() => {
    if (!recording) return
    const handler = (e: KeyboardEvent) => {
      e.preventDefault()
      e.stopPropagation()
      const formatted = formatShortcut(e)
      if (formatted === '') {
        setRecording(null)
        setPendingKeys('')
        return
      }
      setPendingKeys(formatted)
    }
    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [recording])

  const handleTestAI = async () => {
    setAiResult('🔄 连接中...')
    try {
      const res = await invoke<string>('test_ai_connection', { url: aiUrl, model: aiModel, key: aiKey })
      setAiResult(`✅ ${res}`)
    } catch (e: any) {
      setAiResult(`❌ ${e}`)
    }
  }

  const handleSaveAI = async () => {
    await saveAiConfig(aiUrl, aiModel, aiKey)
    setAiResult('✅ 已保存')
  }

  const handleFetchModels = async () => {
    setAiResult('🔄 获取中...')
    try {
      const models = await invoke<string[]>('list_ai_models', { url: aiUrl, key: aiKey })
      setModelList(models)
      setAiResult(`✅ 找到 ${models.length} 个模型`)
    } catch (e: any) {
      setAiResult(`❌ ${e}`)
    }
  }

  const handleTestWD = async () => {
    setWdResult('🔄 连接中...')
    const encrypt = wdEncPass !== ''
    try {
      const res = await invoke<string>('test_webdav', {
        url: wdUrl, user: wdUser, pass: wdPass, path: wdPath,
        encrypt, encPass: wdEncPass,
        syncNotes: wdSyncNotes, syncSummaries: wdSyncSummaries, syncClips: wdSyncClips,
        syncQuestions: wdSyncQuestions, syncFlashcards: wdSyncFlashcards, syncTasks: wdSyncTasks, syncAttachments: wdSyncAttachments,
        syncMode: wdSyncMode,
      })
      setWdResult(`✅ ${res}`)
    } catch (e: any) {
      setWdResult(`❌ ${e}`)
    }
  }

  const handleVerifyWD = async () => {
    setWdResult('🔄 正在验证加密...')
    try {
      const res = await invoke<string>('verify_webdav_encryption', {
        url: wdUrl, user: wdUser, pass: wdPass, path: wdPath,
        encPass: wdEncPass, encAlgorithm: wdEncAlgorithm,
      })
      setWdResult(res)
    } catch (e: any) {
      setWdResult(`❌ ${e}`)
    }
  }

  const handleSyncWD = async () => {
    setSyncProgress(0)
    setSyncMessage('正在启动同步...')
    try {
      const res = await invoke<string>('sync_webdav')
      setWdResult(`✅ ${res}`)
    } catch (e: any) {
      setSyncProgress(null)
      setSyncMessage('')
      setWdResult(`❌ ${e}`)
    }
  }

  const syncPull = useAppStore((s) => s.syncPull)

  const handlePullWD = async () => {
    setSyncProgress(0)
    setSyncMessage('正在拉取云端数据...')
    try {
      await syncPull()
      setWdResult('✅ 拉取完成：设置、数据、附件已从云端同步')
    } catch (e: any) {
      setSyncProgress(null)
      setSyncMessage('')
      setWdResult(`❌ ${e}`)
    }
  }

  const handleSaveWD = async () => {
    const encrypt = wdEncPass !== ''
    await saveWebdavConfig(
      wdUrl, wdUser, wdPass, wdPath, encrypt, wdEncPass, wdEncAlgorithm,
      wdSyncNotes, wdSyncSummaries, wdSyncClips,
      wdSyncQuestions, wdSyncFlashcards, wdSyncTasks, wdSyncAttachments,
      wdSyncMode, wdSyncInterval, wdPullMode, wdSettingsPass, wdSyncSettings, wdAllowUnencrypted,
    )
    setWdResult('✅ 已保存')
  }

  const handleSaveDataDir = async () => {
    try {
      const result = await saveDataDir(dataDir)
      setOldDataDir(result.old_dir)
      setDataDirResult(`✅ 数据已迁移至 ${result.new_dir}`)
    } catch (e: any) {
      setDataDirResult(`❌ ${e}`)
    }
  }

  const handleDeleteOldData = async () => {
    try {
      await deleteFile(oldDataDir)
      setOldDataDir('')
      setDataDirResult('✅ 旧位置数据已清理')
    } catch (e: any) {
      setDataDirResult(`❌ ${e}`)
    }
  }

  const handleOpenOldDir = async () => {
    if (!oldDataDir) return
    await openFileExplorer(oldDataDir)
  }

  return (
    <div className="pn act">
      <div className="st">设置</div>
      <div className="sf">
        <div className="sg2">
          <h4><i className="fas fa-palette" style={{color:'var(--accent)'}}></i> 主题配色</h4>
          <div style={{display:'flex',gap:'12px',padding:'4px 0 8px'}}>
            <label style={{display:'flex',alignItems:'center',gap:'6px',cursor:'pointer',fontSize:'.82rem',color:'var(--text-primary)'}}>
              <input type="radio" name="theme" checked={theme === 'blue'} onChange={() => setTheme('blue')} />
              <span style={{display:'inline-block',width:'14px',height:'14px',borderRadius:'50%',background:'#3b82f6',verticalAlign:'middle'}}></span> Blue
            </label>
            <label style={{display:'flex',alignItems:'center',gap:'6px',cursor:'pointer',fontSize:'.82rem',color:'var(--text-primary)'}}>
              <input type="radio" name="theme" checked={theme === 'indigo'} onChange={() => setTheme('indigo')} />
              <span style={{display:'inline-block',width:'14px',height:'14px',borderRadius:'50%',background:'#6366f1',verticalAlign:'middle'}}></span> Indigo
            </label>
          </div>
        </div>
        <div className="sg2">
          <h4><i className="fas fa-robot" style={{color:'var(--accent)'}}></i> AI 配置</h4>
          <label>API 地址</label>
          <input type="text" value={aiUrl} onChange={(e) => setAiUrl(e.target.value)} placeholder="http://localhost:1234/v1" />
          <label>模型</label>
          <input type="text" value={aiModel} onChange={(e) => setAiModel(e.target.value)} placeholder="qwen2.5-coder-7b-instruct" style={{width:'100%',marginBottom:'6px'}} />
          {modelList.length > 0 && (
            <div style={{display:'flex',flexWrap:'wrap',gap:'4px',marginBottom:'6px'}}>
              {modelList.map((m) => (
                <span key={m} onClick={() => setAiModel(m)} style={{padding:'3px 10px',border:'1px solid var(--border-input)',borderRadius:'10px',fontSize:'.72rem',cursor:'pointer',background: aiModel === m ? 'var(--accent-light)' : 'var(--btn-secondary-bg)',color: aiModel === m ? 'var(--accent)' : 'var(--text-secondary)',transition:'.15s',whiteSpace:'nowrap'}}>{m}</span>
              ))}
            </div>
          )}
          <label>Key（可选）</label>
          <input type="password" value={aiKey} onChange={(e) => setAiKey(e.target.value)} />
          <div style={{display:'flex',gap:'8px',marginTop:'4px',flexWrap:'wrap'}}>
            <button className="btn btn-primary" onClick={handleTestAI}><i className="fas fa-plug"></i> 测试</button>
            <button className="btn btn-secondary" onClick={handleFetchModels}><i className="fas fa-list"></i> 获取模型</button>
            <button className="btn btn-secondary" onClick={handleSaveAI}><i className="fas fa-save"></i> 保存</button>
          </div>
          {aiResult && <div className="tr">{aiResult}</div>}
        </div>
        <div className="sg2">
          <h4><i className="fas fa-cloud" style={{color:'var(--accent)'}}></i> WebDAV 同步 <span style={{fontSize:'.65rem',color:'var(--text-muted)'}}>AES-256-GCM 加密</span></h4>
          <div style={{border:'1px solid var(--border-input)',borderRadius:'8px',padding:'10px 12px',marginBottom:'12px',background:'var(--btn-secondary-bg)'}}>
            <div style={{fontSize:'.74rem',fontWeight:600,color:'var(--text-secondary)',marginBottom:'8px'}}><i className="fas fa-server" style={{marginRight:'4px'}}></i> 服务器连接（登录账户密码）</div>
            <label>服务器地址</label>
            <input type="text" value={wdUrl} onChange={(e) => setWdUrl(e.target.value)} placeholder="https://dav.example.com" />
            <label>用户名</label>
            <input type="text" value={wdUser} onChange={(e) => setWdUser(e.target.value)} />
            <label>账户密码</label>
            <input type="password" value={wdPass} onChange={(e) => setWdPass(e.target.value)} placeholder="WebDAV 账号的登录密码" />
            <label>同步路径</label>
            <input type="text" value={wdPath} onChange={(e) => setWdPath(e.target.value)} />
          </div>
          <div style={{border:'1px solid var(--border-input)',borderRadius:'8px',padding:'10px 12px',marginBottom:'8px',background:'var(--btn-secondary-bg)'}}>
            <div style={{fontSize:'.74rem',fontWeight:600,color:'var(--text-secondary)',marginBottom:'8px'}}><i className="fas fa-lock" style={{marginRight:'4px'}}></i> 数据加密（云端数据加密密码）</div>
            <label>加密密码 <span style={{fontSize:'.68rem',color:'var(--text-muted)',fontWeight:400}}>（留空则不加密，填写后自动启用加密）</span></label>
            <input type="password" value={wdEncPass} onChange={(e) => setWdEncPass(e.target.value)} placeholder="用于加密云端笔记等数据" />
            <div style={{display:'flex',alignItems:'center',gap:'8px',marginTop:'8px'}}>
              <label style={{fontSize:'.72rem',color:'var(--text-secondary)',whiteSpace:'nowrap'}}>加密算法</label>
              <select value={wdEncAlgorithm} onChange={(e) => setWdEncAlgorithm(e.target.value)} style={{padding:'5px 8px',borderRadius:'8px',border:'1px solid var(--border-input)',background:'var(--bg-input)',color:'var(--text-primary)',fontSize:'.75rem'}} disabled={!wdEncPass}>
                <option value="aes256-gcm">AES-256-GCM（推荐，Argon2 密钥）</option>
                <option value="chacha20-poly1305">ChaCha20-Poly1305（Argon2 密钥）</option>
                <option value="aes256-gcm-pbkdf2">AES-256-GCM（PBKDF2 密钥）</option>
              </select>
            </div>
            <div style={{fontSize:'.7rem',color:'var(--text-muted)',marginTop:'8px'}}>
              {wdEncPass ? '🔒 加密已启用（' + wdEncAlgorithm + '）' : '🔓 未加密（未填写加密密码）'}
            </div>
          </div>
          <div style={{display:'flex',alignItems:'center',gap:'8px',padding:'2px 0 8px'}}>
            <label style={{fontSize:'.75rem',color:'var(--text-secondary)',whiteSpace:'nowrap'}}>自动同步间隔</label>
            <input type="number" min="0" value={wdSyncInterval} onChange={(e) => setWdSyncInterval(Math.max(0, parseInt(e.target.value) || 0))} style={{width:'60px',textAlign:'center'}} />
            <span style={{fontSize:'.72rem',color:'var(--text-muted)'}}>分钟（0=关闭）</span>
          </div>
          <div style={{margin:'8px 0 4px'}}>
            <div style={{display:'flex',alignItems:'center',gap:'6px',cursor:'pointer',userSelect:'none'}} onClick={() => setScopeExpanded(!scopeExpanded)}>
              <i className={`fas fa-chevron-${scopeExpanded ? 'down' : 'right'}`} style={{fontSize:'.68rem',color:'var(--text-muted)'}}></i>
              <span style={{fontSize:'.78rem',fontWeight:600,color:'var(--text-secondary)'}}>同步范围</span>
            </div>
            {scopeExpanded && (
              <div style={{display:'flex',flexWrap:'wrap',gap:'6px',padding:'6px 0 4px 14px'}}>
                {SYNC_ITEMS.map((item) => {
                  const checkedMap: Record<SyncKey, boolean> = {
                    sync_notes: wdSyncNotes,
                    sync_summaries: wdSyncSummaries,
                    sync_clips: wdSyncClips,
                    sync_questions: wdSyncQuestions,
                    sync_flashcards: wdSyncFlashcards,
                    sync_tasks: wdSyncTasks,
                    sync_attachments: wdSyncAttachments,
                  }
                  const setterMap: Record<SyncKey, React.Dispatch<React.SetStateAction<boolean>>> = {
                    sync_notes: setWdSyncNotes,
                    sync_summaries: setWdSyncSummaries,
                    sync_clips: setWdSyncClips,
                    sync_questions: setWdSyncQuestions,
                    sync_flashcards: setWdSyncFlashcards,
                    sync_tasks: setWdSyncTasks,
                    sync_attachments: setWdSyncAttachments,
                  }
                  const checked = checkedMap[item.key]
                  const setter = setterMap[item.key]
                  return (
                    <label key={item.key} style={{display:'flex',alignItems:'center',gap:'4px',fontSize:'.75rem',color:'var(--text-primary)',cursor:'pointer'}}>
                      <input type="checkbox" checked={checked} onChange={(e) => setter(e.target.checked)} />
                      {item.label}
                    </label>
                  )
                })}
              </div>
            )}
            {wdEncPass === '' && (
              <div style={{display:'flex',alignItems:'center',gap:'6px',padding:'6px 0 4px 14px'}}>
                <input type="checkbox" checked={wdAllowUnencrypted} onChange={(e) => setWdAllowUnencrypted(e.target.checked)} />
                <span style={{fontSize:'.75rem',color:'var(--text-primary)'}}>允许明文上传/下载附件</span>
                <span style={{fontSize:'.7rem',color:'var(--text-secondary)'}}>（未设置加密密码时必开，否则无法同步资料文件）</span>
              </div>
            )}
          </div>
          <div style={{display:'flex',alignItems:'center',gap:'12px',padding:'4px 0 8px'}}>
            <span style={{fontSize:'.75rem',color:'var(--text-secondary)'}}>同步模式</span>
            <label style={{display:'flex',alignItems:'center',gap:'4px',fontSize:'.75rem',color:'var(--text-primary)',cursor:'pointer'}}>
              <input type="radio" name="syncMode" checked={wdSyncMode === 'upload'} onChange={() => setWdSyncMode('upload')} />
              单向上传
            </label>
            <label style={{display:'flex',alignItems:'center',gap:'4px',fontSize:'.75rem',color:'var(--text-primary)',cursor:'pointer'}}>
              <input type="radio" name="syncMode" checked={wdSyncMode === 'merge'} onChange={() => setWdSyncMode('merge')} />
              双向合并
            </label>
          </div>
          <div style={{display:'flex',alignItems:'center',gap:'12px',padding:'4px 0 8px'}}>
            <span style={{fontSize:'.75rem',color:'var(--text-secondary)'}}>拉取模式</span>
            <label style={{display:'flex',alignItems:'center',gap:'4px',fontSize:'.75rem',color:'var(--text-primary)',cursor:'pointer'}}>
              <input type="radio" name="pullMode" checked={wdPullMode === 'add'} onChange={() => setWdPullMode('add')} />
              仅新增
            </label>
            <label style={{display:'flex',alignItems:'center',gap:'4px',fontSize:'.75rem',color:'var(--text-primary)',cursor:'pointer'}}>
              <input type="radio" name="pullMode" checked={wdPullMode === 'overwrite'} onChange={() => setWdPullMode('overwrite')} />
              覆盖
            </label>
            <span style={{fontSize:'.65rem',color:'var(--text-muted)'}}>仅新增=添加云端多出的条目；覆盖=完全替换本地</span>
          </div>
          <div style={{border:'1px solid var(--accent)',borderRadius:'8px',padding:'10px 12px',margin:'12px 0 8px',background:'var(--accent-light)'}}>
            <div style={{display:'flex',alignItems:'center',justifyContent:'space-between',gap:'8px',cursor:'pointer'}} onClick={() => setWdSyncSettings(!wdSyncSettings)}>
              <span style={{fontSize:'.82rem',fontWeight:700,color:'var(--text-primary)'}}><i className="fas fa-sliders-h" style={{marginRight:'6px',color:'var(--accent)'}}></i> 软件设置同步</span>
              <label style={{display:'flex',alignItems:'center',gap:'6px',cursor:'pointer',fontSize:'.76rem',color:'var(--text-secondary)'}}>
                <input type="checkbox" checked={wdSyncSettings} onChange={(e) => setWdSyncSettings(e.target.checked)} />
                {wdSyncSettings ? '已开启' : '已关闭'}
              </label>
            </div>
            <div style={{fontSize:'.68rem',color:'var(--text-muted)',margin:'4px 0 8px'}}>开启后会把 AI 配置、快捷键等软件设置加密同步到云端（需先填写下方密码）</div>
            <label>设置加密密码 <span style={{fontSize:'.68rem',color:'var(--text-muted)',fontWeight:400}}>（留空则使用「数据加密密码」，设置将始终 AES-256-GCM 加密存储到云端）</span></label>
            <input type="password" value={wdSettingsPass} onChange={(e) => setWdSettingsPass(e.target.value)} placeholder="用于加密云端的软件设置" disabled={!wdSyncSettings} />
          </div>
          <div style={{display:'flex',gap:'8px',flexWrap:'wrap',marginTop:'8px'}}>
            <button className="btn btn-primary" onClick={handleTestWD} disabled={syncProgress !== null}><i className="fas fa-plug"></i> 测试</button>
            <button className="btn btn-primary" onClick={handleSyncWD} disabled={syncProgress !== null}><i className="fas fa-sync"></i> {syncProgress !== null ? '同步中...' : '同步'}</button>
            <button className="btn btn-primary" onClick={handlePullWD} disabled={syncProgress !== null} style={{background:'var(--accent)'}}><i className="fas fa-download"></i> 拉取</button>
            <button className="btn btn-secondary" onClick={handleVerifyWD} disabled={syncProgress !== null} style={{fontSize:'.72rem'}}><i className="fas fa-shield-alt"></i> 验证加密</button>
            <button className="btn btn-secondary" onClick={handleSaveWD} disabled={syncProgress !== null}><i className="fas fa-save"></i> 保存</button>
          </div>
          {syncProgress !== null && (
            <div className="sync-progress">
              <div className="sync-progress-bar"><div className="sync-progress-fill" style={{width:`${syncProgress}%`}}></div></div>
              <span className="sync-progress-text">{syncProgress}%</span>
              <span style={{fontSize:'.68rem',color:'var(--text-muted)'}}>{syncMessage}</span>
            </div>
          )}
          {wdResult && !syncProgress && <div className="tr">{wdResult}</div>}
        </div>
        <div className="sg2">
          <h4><i className="fas fa-database" style={{color:'var(--accent)'}}></i> 数据文件存放路径</h4>
          <label>更改后将自动复制数据到新位置</label>
          <div style={{display:'flex',gap:'6px',alignItems:'center',marginBottom:'8px'}}>
            <input type="text" value={dataDir} onChange={(e) => setDataDir(e.target.value)} placeholder="留空使用默认路径" style={{flex:1}} />
            <button className="btn btn-secondary" style={{padding:'7px 14px',whiteSpace:'nowrap',flexShrink:0}} onClick={async () => {
              const dir = await open({ directory: true, multiple: false, title: '选择数据文件存放路径' })
              if (dir) setDataDir(dir)
            }}><i className="fas fa-folder-open"></i> 选择</button>
          </div>
          <div style={{display:'flex',gap:'8px',marginTop:'4px',flexWrap:'wrap'}}>
            <button className="btn btn-primary" onClick={handleSaveDataDir}><i className="fas fa-save"></i> 保存</button>
          </div>
          {dataDirResult && (
            <div style={{marginTop:'6px',fontSize:'.72rem',color:'var(--text-primary)'}}>
              <div className="tr">{dataDirResult}</div>
              {oldDataDir && (
                <div style={{display:'flex',gap:'8px',marginTop:'6px'}}>
                  <button className="btn btn-secondary" style={{padding:'3px 10px',fontSize:'.68rem'}} onClick={handleOpenOldDir}>
                    <i className="fas fa-folder-open"></i> 打开旧位置文件夹
                  </button>
                  <button className="btn btn-secondary" style={{padding:'3px 10px',fontSize:'.68rem',color:'var(--priority-high)'}} onClick={handleDeleteOldData}>
                    <i className="fas fa-trash"></i> 删除旧位置数据
                  </button>
                </div>
              )}
            </div>
          )}
        </div>
        <div className="sg2">
          <h4><i className="fas fa-folder-open" style={{color:'var(--accent)'}}></i> 资料存放目录</h4>
          <label>自定义资料文件存放路径（留空使用默认位置）</label>
          <div style={{display:'flex',gap:'6px',alignItems:'center',marginBottom:'8px'}}>
            <input type="text" value={attDir} onChange={(e) => setAttDir(e.target.value)} placeholder="留空使用默认路径 (AppData/attachments)" style={{flex:1}} />
            <button className="btn btn-secondary" style={{padding:'7px 14px',whiteSpace:'nowrap',flexShrink:0}} onClick={async () => {
              const dir = await open({ directory: true, multiple: false, title: '选择资料存放目录' })
              if (dir) setAttDir(dir)
            }}><i className="fas fa-folder-open"></i> 选择</button>
          </div>
          <div style={{display:'flex',gap:'8px',marginTop:'4px'}}>
            <button className="btn btn-primary" onClick={async () => {
              try {
                const res = await saveAttachmentDir(attDir)
                setAttMigrate(res)
                setAttDirResult('✅ 已保存')
                setTimeout(() => setAttDirResult(''), 2000)
              } catch (e: any) {
                setAttDirResult('❌ ' + (typeof e === 'string' ? e : (e?.message || '保存失败')))
              }
            }}><i className="fas fa-save"></i> 保存</button>
            {attDirResult && <span className="tr" style={{fontSize:'.72rem',color:'var(--text-primary)',alignSelf:'center'}}>{attDirResult}</span>}
          </div>
          {attMigrate && (attMigrate.moved > 0 || attMigrate.skipped > 0) && (
            <div style={{marginTop:'8px',fontSize:'.74rem',color:'var(--text-primary)',background:'var(--bg-sidebar)',border:'1px solid var(--border)',borderRadius:'10px',padding:'8px 10px'}}>
              <div style={{fontWeight:600,marginBottom:'4px'}}><i className="fas fa-exchange-alt" style={{marginRight:'4px',color:'var(--accent)'}}></i> 迁移报告</div>
              <div>· 已从旧目录迁移 <b>{attMigrate.moved}</b> 个文件到新目录</div>
              {attMigrate.skipped > 0 && (
                <div style={{color:'var(--priority-high)'}}>· {attMigrate.skipped} 个文件因新目录已存在同名文件被跳过（已备份到下方目录，未覆盖新文件）</div>
              )}
              {attMigrate.backup_dir && (
                <div style={{marginTop:'4px',display:'flex',alignItems:'center',gap:'8px',flexWrap:'wrap'}}>
                  <span style={{fontSize:'.7rem',color:'var(--text-muted)'}}>备份目录：{attMigrate.backup_dir}</span>
                  <button className="btn btn-secondary" style={{padding:'2px 10px',fontSize:'.68rem'}} onClick={async () => {
                    await deleteAttachmentDirBackup(attMigrate.backup_dir)
                    setAttMigrate({ ...attMigrate, backup_dir: '', skipped: 0 })
                  }}><i className="fas fa-trash"></i> 删除备份</button>
                </div>
              )}
            </div>
          )}
          <div style={{display:'flex',alignItems:'center',justifyContent:'space-between',gap:'8px',marginTop:'12px',borderTop:'1px solid var(--border)',paddingTop:'10px',cursor:'pointer'}} onClick={async () => {
            const next = !attMoveMode
            setAttMoveMode(next)
            try {
              await saveAttachmentMoveMode(next)
              setAttDirResult('✅ 已保存')
              setTimeout(() => setAttDirResult(''), 2000)
            } catch (e: any) {
              setAttMoveMode(!next)
              setAttDirResult('❌ ' + (typeof e === 'string' ? e : (e?.message || '保存失败')))
            }
          }}>
            <div>
              <div style={{fontSize:'.78rem',fontWeight:600,color:'var(--text-primary)'}}>拖拽文件时移动而非复制</div>
              <div style={{fontSize:'.7rem',color:'var(--text-muted)',marginTop:'2px'}}>开启后，拖入的资料会从原位置移入附件目录（原文件不再保留）；关闭则保留原文件只复制一份。</div>
            </div>
            <input type="checkbox" checked={attMoveMode} readOnly onClick={(e) => e.stopPropagation()} onChange={() => {}} />
          </div>
        </div>
        <div className="sg2">
          <h4><i className="fas fa-keyboard" style={{color:'var(--accent)'}}></i> 快捷键</h4>
          {SHORTCUT_KEYS.map(({ key, label }) => (
            <div key={key} style={{display:'flex',alignItems:'center',gap:'8px',padding:'4px 0'}}>
              <span style={{flex:1,fontSize:'.78rem',color:'var(--text-secondary)'}}>{label}</span>
              <kbd style={{minWidth:'100px',textAlign:'center',background: recording === key ? 'var(--status-progress-bg)' : 'var(--btn-secondary-bg)',padding:'3px 10px',borderRadius:'6px',fontSize:'.72rem',fontFamily:'monospace',color:'var(--text-primary)',whiteSpace:'nowrap',border:'1px solid var(--border)'}}>
                {recording === key ? (pendingKeys || '按下组合键...') : shortcuts[key]}
              </kbd>
              <button className="btn btn-secondary" style={{padding:'2px 10px',fontSize:'.68rem',minWidth:'44px'}} onClick={() => {
                if (recording === key) {
                  if (pendingKeys) setShortcuts((s) => ({ ...s, [key]: pendingKeys }))
                  setRecording(null)
                  setPendingKeys('')
                } else {
                  setRecording(key)
                  setPendingKeys('')
                }
              }}>
                {recording === key ? (pendingKeys ? '确认' : '...') : '录制'}
              </button>
              {recording === key && (
                <button className="btn btn-ghost" style={{padding:'2px 8px',fontSize:'.68rem'}} onClick={() => { setRecording(null); setPendingKeys('') }}>取消</button>
              )}
            </div>
          ))}
          <div style={{display:'flex',gap:'8px',marginTop:'8px'}}>
            <button className="btn btn-primary" onClick={async () => {
              await saveShortcuts(shortcuts.send_note, shortcuts.quick_note)
              setScResult('✅ 快捷键已保存')
              setTimeout(() => setScResult(''), 2000)
            }}><i className="fas fa-save"></i> 保存快捷键</button>
            {scResult && <span className="tr" style={{fontSize:'.72rem',color:'var(--text-primary)',alignSelf:'center'}}>{scResult}</span>}
          </div>
        </div>
      </div>
    </div>
  )
}
