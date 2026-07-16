import { useState, useRef, useMemo, useEffect } from 'react'
import { useAppStore } from '../store/useAppStore'
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'
import type { Attachment, Note, RemoteAttachment } from '../types'

function formatSize(bytes: number): string {
  if (bytes < 1024) return bytes + ' B'
  if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB'
  return (bytes / (1024 * 1024)).toFixed(1) + ' MB'
}

export default function FilesPanel() {
  const data = useAppStore((s) => s.data)
  const addAttachment = useAppStore((s) => s.addAttachment)
  const deleteAttachment = useAppStore((s) => s.deleteAttachment)
  const linkNoteAttachment = useAppStore((s) => s.linkNoteAttachment)
  const unlinkNoteAttachment = useAppStore((s) => s.unlinkNoteAttachment)
  const moveAttachment = useAppStore((s) => s.moveAttachment)
  const openAttachmentFolder = useAppStore((s) => s.openAttachmentFolder)
  const [uploading, setUploading] = useState(false)
  const [dragOver, setDragOver] = useState(false)
  const [newFolder, setNewFolder] = useState('')
  const [activeFolder, setActiveFolder] = useState('')
  const fileRef = useRef<HTMLInputElement>(null)
  const [linkModalAttId, setLinkModalAttId] = useState<number | null>(null)
  const [linkSearch, setLinkSearch] = useState('')
  const [moveTarget, setMoveTarget] = useState<{ id: number; show: boolean }>({ id: 0, show: false })
  const [remoteAtts, setRemoteAtts] = useState<RemoteAttachment[] | null>(null)
  const [remoteLoading, setRemoteLoading] = useState(false)
  const [remoteError, setRemoteError] = useState('')
  const listRemoteAttachments = useAppStore((s) => s.listRemoteAttachments)

  const loadRemote = async () => {
    setRemoteLoading(true)
    setRemoteError('')
    try {
      const list = await listRemoteAttachments()
      setRemoteAtts(list)
    } catch (e: any) {
      setRemoteError(typeof e === 'string' ? e : (e?.message || '加载云端附件失败'))
      setRemoteAtts([])
    } finally {
      setRemoteLoading(false)
    }
  }
  const searchRef = useRef<HTMLInputElement>(null)

  useEffect(() => {
    if (linkModalAttId !== null) {
      setLinkSearch('')
      setTimeout(() => searchRef.current?.focus(), 50)
    }
  }, [linkModalAttId])

  useEffect(() => {
    const w = getCurrentWebviewWindow()
    let cleanup: (() => void) | null = null
    w.onDragDropEvent(async (event) => {
      if (event.payload.type === 'over' || event.payload.type === 'enter') {
        setDragOver(true)
      } else if (event.payload.type === 'leave') {
        setDragOver(false)
      } else if (event.payload.type === 'drop') {
        setDragOver(false)
        setUploading(true)
        for (const path of event.payload.paths) {
          try {
            const [name, bytes] = await invoke<[string, number[]]>('read_file', { path })
            await addAttachment(name, bytes)
          } catch (e) {
            console.error('上传失败', e)
          }
        }
        setUploading(false)
      }
    }).then((fn) => { cleanup = fn })
    return () => { cleanup?.() }
  }, [])

  const attachments: Attachment[] = data?.attachments || []
  const notes: Note[] = data?.notes || []

  const folders = useMemo(() => {
    const set = new Set<string>()
    set.add('')
    attachments.forEach((a) => { if (a.folder) set.add(a.folder) })
    return Array.from(set).sort()
  }, [attachments])

  const filtered = activeFolder ? attachments.filter((a) => a.folder === activeFolder) : attachments
  const folderCounts = useMemo(() => {
    const m: Record<string, number> = {}
    attachments.forEach((a) => { m[a.folder || ''] = (m[a.folder || ''] || 0) + 1 })
    return m
  }, [attachments])

  const processFiles = async (files: FileList) => {
    setUploading(true)
    for (let i = 0; i < files.length; i++) {
      const file = files[i]
      const buf = await file.arrayBuffer()
      const bytes = Array.from(new Uint8Array(buf))
      try {
        await addAttachment(file.name, bytes)
      } catch (e) {
        console.error('上传失败', e)
      }
    }
    setUploading(false)
  }

  const handleUpload = async () => {
    const files = fileRef.current?.files
    if (!files || files.length === 0) return
    await processFiles(files)
    if (fileRef.current) fileRef.current.value = ''
  }

  const handleDelete = async (id: number) => {
    if (!confirm('确定删除此文件？')) return
    try {
      await deleteAttachment(id)
    } catch (e) {
      console.error('删除失败', e)
    }
  }

  const handleLink = async (attId: number, noteId: number) => {
    try {
      await linkNoteAttachment(attId, noteId)
    } catch (e) {
      console.error('关联失败', e)
    }
  }

  const handleUnlink = async (attId: number, noteId: number) => {
    try {
      await unlinkNoteAttachment(attId, noteId)
    } catch (e) {
      console.error('取消关联失败', e)
    }
  }

  const handleMove = async (id: number, folder: string) => {
    try {
      await moveAttachment(id, folder)
      setMoveTarget({ id: 0, show: false })
    } catch (e) {
      console.error('移动失败', e)
    }
  }

  const handleCreateFolder = async () => {
    const name = newFolder.trim()
    if (!name || folders.includes(name)) return
    setActiveFolder(name)
    setNewFolder('')
  }

  const handleOpenFolder = async (id: number) => {
    try {
      await openAttachmentFolder(id)
    } catch (e) {
      console.error('打开文件夹失败', e)
    }
  }

  const currentAtt = linkModalAttId !== null ? attachments.find((a) => a.id === linkModalAttId) : null
  const sortedNotes = useMemo(() => {
    return [...notes].sort((a, b) => b.id - a.id)
  }, [notes])
  const filteredNotes = linkSearch
    ? sortedNotes.filter((n) => n.text.includes(linkSearch))
    : sortedNotes

  const handleToggleLink = async (noteId: number) => {
    if (!currentAtt) return
    if (currentAtt.note_ids.includes(noteId)) {
      await handleUnlink(currentAtt.id, noteId)
    } else {
      await handleLink(currentAtt.id, noteId)
    }
  }

  return (
    <div className="pn act">
      <div className="st">资料上传</div>
      <div className={'file-upload-bar' + (dragOver ? ' drag-over' : '')}>
        <input type="file" ref={fileRef} multiple onChange={handleUpload} style={{ display: 'none' }} />
        <button className="btn btn-primary" onClick={() => fileRef.current?.click()} disabled={uploading}>
          <i className="fas fa-upload"></i> {uploading ? '上传中...' : '选择文件上传'}
        </button>
        <span className="file-hint">{dragOver ? '释放以上传文件' : '支持任意文件类型，也可拖拽文件到此处'}</span>
        <button className="btn btn-secondary" style={{ marginLeft: 'auto', padding: '6px 12px', fontSize: '.74rem' }} onClick={loadRemote} disabled={remoteLoading}>
          <i className="fas fa-cloud-download-alt"></i> {remoteLoading ? '查询中...' : '查看云端附件'}
        </button>
      </div>
      {remoteAtts !== null && (
        <div className="cloud-att-list">
          <div className="cloud-att-head">
            <span><i className="fas fa-cloud" style={{ color: 'var(--accent)', marginRight: 6 }}></i>云端附件（{remoteAtts.length}）</span>
            <button className="btn btn-ghost" style={{ padding: '2px 8px', fontSize: '.72rem' }} onClick={loadRemote} disabled={remoteLoading}>
              <i className="fas fa-sync"></i> 刷新
            </button>
          </div>
          {remoteError && <div className="em" style={{ color: 'var(--priority-high)' }}>{remoteError}</div>}
          {!remoteError && remoteAtts.length === 0 && <div className="em">云端暂无附件（已确认上传成功的文件会显示在这里）</div>}
          {remoteAtts.map((a) => (
            <div className="cloud-att-item" key={a.filename}>
              <i className="fas fa-file"></i>
              <span className="cloud-att-name">{a.filename}</span>
              <span className="cloud-att-size">{a.size >= 1024 * 1024 ? (a.size / (1024 * 1024)).toFixed(1) + ' MB' : a.size >= 1024 ? (a.size / 1024).toFixed(1) + ' KB' : a.size + ' B'}</span>
              {a.exists_local
                ? <span className="cloud-att-badge local"><i className="fas fa-check"></i> 本地已存在</span>
                : <span className="cloud-att-badge remote">仅云端</span>}
            </div>
          ))}
        </div>
      )}
      <div className="file-folder-bar">
        <div className="file-folders">
          <span
            className={'file-folder-tab' + (activeFolder === '' ? ' active' : '')}
            onClick={() => setActiveFolder('')}
          >全部 ({attachments.length})</span>
          {folders.filter((f) => f).map((f) => (
            <span
              key={f}
              className={'file-folder-tab' + (activeFolder === f ? ' active' : '')}
              onClick={() => setActiveFolder(f)}
            ><i className="fas fa-folder"></i> {f} ({folderCounts[f] || 0})</span>
          ))}
        </div>
        <div className="file-folder-add">
          <input
            value={newFolder}
            onChange={(e) => setNewFolder(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && handleCreateFolder()}
            placeholder="新建文件夹..."
          />
          <button className="btn btn-secondary" style={{padding:'3px 10px',fontSize:'.68rem'}} onClick={handleCreateFolder}>
            <i className="fas fa-plus"></i>
          </button>
        </div>
      </div>
      {filtered.length === 0 ? (
        <div className="em">暂无资料{activeFolder ? '（' + activeFolder + '）' : ''}</div>
      ) : (
        <div className="file-list">
          {filtered.map((a) => (
            <div className="file-item" key={a.id}>
              <div className="file-icon"><i className="fas fa-file"></i></div>
              <div className="file-info">
                <div className="file-name">{a.filename}</div>
                <div className="file-meta">
                  <span>{formatSize(a.size)}</span>
                  <span>{a.created}</span>
                  {a.folder && <span><i className="fas fa-folder"></i> {a.folder}</span>}
                </div>
                <div className="file-notes">
                  {a.note_ids.map((nid) => {
                    const note = notes.find((n) => n.id === nid)
                    return note ? (
                      <span key={nid} className="file-note-tag">
                        {note.text.slice(0, 20)}
                        <i className="fas fa-times" onClick={() => handleUnlink(a.id, nid)}></i>
                      </span>
                    ) : null
                  })}
                </div>
              </div>
              <div className="file-actions">
                <button className="btn btn-secondary" title="打开所在文件夹" onClick={() => handleOpenFolder(a.id)}>
                  <i className="fas fa-folder-open"></i>
                </button>
                <button className="btn btn-secondary" title="关联笔记" onClick={() => setLinkModalAttId(a.id)}>
                  <i className="fas fa-link"></i>
                </button>
                <button className="btn btn-secondary" onClick={() => setMoveTarget({ id: a.id, show: moveTarget.id === a.id ? !moveTarget.show : true })}>
                  <i className="fas fa-arrow-right"></i>
                </button>
                <button className="btn btn-secondary" onClick={() => handleDelete(a.id)}>
                  <i className="fas fa-trash"></i>
                </button>
              </div>
              {moveTarget.show && moveTarget.id === a.id && (
                <div className="file-link-picker" style={{right:'80px'}}>
                  {folders.map((f) => (
                    <div className="file-link-item" key={f} onClick={() => handleMove(a.id, f)}>
                      <span>{f || '(根目录)'}</span>
                      {a.folder === f && <i className="fas fa-check" style={{ color: 'var(--accent)' }}></i>}
                    </div>
                  ))}
                </div>
              )}
            </div>
          ))}
        </div>
      )}

      {linkModalAttId !== null && currentAtt && (
        <div className="mo op" onClick={() => setLinkModalAttId(null)}>
          <div className="mb" onClick={(e) => e.stopPropagation()} style={{maxHeight:'80vh',display:'flex',flexDirection:'column'}}>
            <div style={{display:'flex',justifyContent:'space-between',alignItems:'center',marginBottom:'10px'}}>
              <strong style={{fontSize:'.9rem',color:'var(--text-primary)'}}>关联笔记</strong>
              <span style={{fontSize:'.75rem',color:'var(--text-muted)'}}>{currentAtt.filename} · 已关联 {currentAtt.note_ids.length} 条</span>
              <button className="btn btn-ghost" style={{padding:'2px 8px',fontSize:'.8rem'}} onClick={() => setLinkModalAttId(null)}>
                <i className="fas fa-times"></i>
              </button>
            </div>
            <input
              ref={searchRef}
              value={linkSearch}
              onChange={(e) => setLinkSearch(e.target.value)}
              placeholder="搜索笔记..."
              style={{width:'100%',padding:'7px 10px',border:'1px solid var(--border-input)',borderRadius:'10px',fontSize:'.82rem',outline:'none',background:'var(--bg-input)',color:'var(--text-primary)',marginBottom:'8px'}}
            />
            <div style={{flex:1,overflowY:'auto',minHeight:0,maxHeight:'50vh'}}>
              {filteredNotes.length === 0 ? (
                <div style={{textAlign:'center',padding:'20px',color:'var(--text-muted)',fontSize:'.8rem'}}>暂无匹配笔记</div>
              ) : (
                filteredNotes.map((n) => {
                  const linked = currentAtt.note_ids.includes(n.id)
                  return (
                    <div
                      key={n.id}
                      onClick={() => handleToggleLink(n.id)}
                      style={{
                        display:'flex',alignItems:'center',gap:'8px',padding:'6px 8px',cursor:'pointer',borderRadius:'8px',transition:'background .1s',
                        background: linked ? 'var(--filtered-highlight-bg)' : 'transparent',
                        marginBottom:'2px'
                      }}
                      onMouseEnter={(e) => (e.currentTarget.style.background = linked ? 'var(--filtered-highlight-bg)' : 'var(--btn-secondary-bg)')}
                      onMouseLeave={(e) => (e.currentTarget.style.background = linked ? 'var(--filtered-highlight-bg)' : 'transparent')}
                    >
                      <i
                        className={'fas ' + (linked ? 'fa-check-circle' : 'fa-circle')}
                        style={{color: linked ? 'var(--accent)' : 'var(--text-muted)',fontSize:'.85rem',width:'18px',textAlign:'center'}}
                      ></i>
                      <div style={{flex:1,minWidth:0}}>
                        <div style={{fontSize:'.82rem',color:'var(--text-primary)',overflow:'hidden',textOverflow:'ellipsis',whiteSpace:'nowrap'}}>{n.text.slice(0, 60)}</div>
                        <div style={{fontSize:'.65rem',color:'var(--text-muted)',marginTop:'1px'}}>{n.date}</div>
                      </div>
                    </div>
                  )
                })
              )}
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
