import { useState, useRef, useMemo, useEffect } from 'react'
import { useAppStore } from '../store/useAppStore'
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'
import type { Attachment, Note, RemoteAttachment, AttachmentFileStatus } from '../types'

function formatSize(bytes: number): string {
  if (bytes < 1024) return bytes + ' B'
  if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB'
  return (bytes / (1024 * 1024)).toFixed(1) + ' MB'
}

export default function FilesPanel() {
  const data = useAppStore((s) => s.data)
  const addAttachment = useAppStore((s) => s.addAttachment)
  const addAttachmentFromPath = useAppStore((s) => s.addAttachmentFromPath)
  const deleteAttachment = useAppStore((s) => s.deleteAttachment)
  const linkNoteAttachment = useAppStore((s) => s.linkNoteAttachment)
  const unlinkNoteAttachment = useAppStore((s) => s.unlinkNoteAttachment)
  const moveAttachment = useAppStore((s) => s.moveAttachment)
  const createAttachmentFolder = useAppStore((s) => s.createAttachmentFolder)
  const renameAttachmentFolder = useAppStore((s) => s.renameAttachmentFolder)
  const deleteAttachmentFolder = useAppStore((s) => s.deleteAttachmentFolder)
  const openAttachmentFolder = useAppStore((s) => s.openAttachmentFolder)
  const openAttachmentFolderByName = useAppStore((s) => s.openAttachmentFolderByName)
  const checkAttachmentFiles = useAppStore((s) => s.checkAttachmentFiles)
  const [uploading, setUploading] = useState(false)
  const [dragOver, setDragOver] = useState(false)
  const [newFolder, setNewFolder] = useState('')
  const [fileSearch, setFileSearch] = useState('')
  const [activeFolder, setActiveFolder] = useState<string>('') // '' = 未分类, '__all__' = 全部
  const ACTIVE_ALL = '__all__'
  const fileRef = useRef<HTMLInputElement>(null)
  const [linkModalAttId, setLinkModalAttId] = useState<number | null>(null)
  const [linkSearch, setLinkSearch] = useState('')
  const [remoteAtts, setRemoteAtts] = useState<RemoteAttachment[] | null>(null)
  const [remoteLoading, setRemoteLoading] = useState(false)
  const [remoteError, setRemoteError] = useState('')
  const listRemoteAttachments = useAppStore((s) => s.listRemoteAttachments)
  const uploadAttachment = useAppStore((s) => s.uploadAttachment)
  const downloadAttachment = useAppStore((s) => s.downloadAttachment)
  const verifyAttachmentEncryption = useAppStore((s) => s.verifyAttachmentEncryption)
  const debugListRemoteAttachments = useAppStore((s) => s.debugListRemoteAttachments)
  const [debugInfo, setDebugInfo] = useState<string>('')
  const [debugLoading, setDebugLoading] = useState(false)
  const [verifyResult, setVerifyResult] = useState('')
  const [verifyLoading, setVerifyLoading] = useState(false)
  const [missingFiles, setMissingFiles] = useState<Set<number>>(new Set())
  const [uploadingIds, setUploadingIds] = useState<Set<number>>(new Set())
  const [downloadingNames, setDownloadingNames] = useState<Set<string>>(new Set())
  const [selectMode, setSelectMode] = useState(false)
  const [selected, setSelected] = useState<Set<number>>(new Set())
  const [batchBusy, setBatchBusy] = useState(false)
  const [manageOpen, setManageOpen] = useState(false)
  const [addingFolder, setAddingFolder] = useState(false)
  const [renameFolder, setRenameFolder] = useState<string | null>(null)
  const [renameVal, setRenameVal] = useState('')
  const [movePicker, setMovePicker] = useState<{ ids: number[] } | null>(null)
  const [delFolder, setDelFolder] = useState<string | null>(null)
  const [deleteModal, setDeleteModal] = useState<{ ids: number[]; single: boolean } | null>(null)

  const wc = data?.webdav_config
  const webdavConfigured = !!(wc && wc.url && wc.user && wc.pass)

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

  const handleVerifyEncryption = async () => {
    setVerifyLoading(true)
    setVerifyResult('')
    try {
      const msg = await verifyAttachmentEncryption()
      setVerifyResult(msg)
    } catch (e: any) {
      setVerifyResult(typeof e === 'string' ? e : (e?.message || '校验失败'))
    } finally {
      setVerifyLoading(false)
    }
  }

  const handleDebug = async () => {
    setDebugLoading(true)
    setDebugInfo('')
    try {
      const info = await debugListRemoteAttachments()
      setDebugInfo(JSON.stringify(info, null, 2))
    } catch (e: any) {
      setDebugInfo(typeof e === 'string' ? e : (e?.message || '调试失败'))
    } finally {
      setDebugLoading(false)
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
    let cancelled = false
    const run = async () => {
      try {
        const statuses: AttachmentFileStatus[] = await checkAttachmentFiles()
        if (cancelled) return
        const miss = new Set<number>()
        statuses.forEach((s) => { if (!s.exists) miss.add(s.id) })
        setMissingFiles(miss)
      } catch {
        // 忽略校验失败，不阻塞面板
      }
    }
    run()
    return () => { cancelled = true }
  }, [checkAttachmentFiles, data])

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
        let failed = 0
        for (const path of event.payload.paths) {
          try {
            await addAttachmentFromPath(path)
          } catch (e: any) {
            console.error('上传失败', e)
            failed++
          }
        }
        setUploading(false)
        if (failed > 0) {
          alert(`有 ${failed} 个文件上传失败，请查看控制台日志`)
        }
      }
    }).then((fn) => { cleanup = fn })
    return () => { cleanup?.() }
  }, [])

  const attachments: Attachment[] = data?.attachments || []
  const notes: Note[] = data?.notes || []

  const folders = useMemo(() => {
    const set = new Set<string>()
    ;(data?.folders || []).forEach((f) => { if (f) set.add(f) })
    attachments.forEach((a) => { if (a.folder) set.add(a.folder) })
    return Array.from(set).sort()
  }, [attachments, data?.folders])

  const FOLDER_COLORS: Record<string, string> = {
    '工作': '#aa3bff', '学习': '#2bb3a3', '项目': '#e0913d',
  }
  const folderColor = (f: string) => FOLDER_COLORS[f] || '#aa3bff'

  const filtered = (() => {
    const byFolder = activeFolder === ACTIVE_ALL
      ? attachments
      : activeFolder === ''
        ? attachments.filter((a) => !a.folder)
        : attachments.filter((a) => a.folder === activeFolder)
    const kw = fileSearch.trim().toLowerCase()
    if (!kw) return byFolder
    return byFolder.filter((a) => a.filename.toLowerCase().includes(kw))
  })()
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
    setDeleteModal({ ids: [id], single: true })
  }

  const confirmDelete = async () => {
    if (!deleteModal) return
    const ids = deleteModal.ids
    setDeleteModal(null)
    for (const id of ids) {
      try {
        await deleteAttachment(id)
      } catch (e: any) {
        console.error('删除失败', e)
        alert(typeof e === 'string' ? e : (e?.message || '删除失败'))
      }
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

  const doMove = async (folder: string) => {
    if (!movePicker) return
    setMovePicker(null)
    let failed = 0
    for (const id of movePicker.ids) {
      try { await moveAttachment(id, folder) } catch { failed++ }
    }
    if (failed > 0) alert(`有 ${failed} 个移动失败`)
  }

  const createAndMove = async (name: string) => {
    try {
      await createAttachmentFolder(name)
    } catch (e: any) {
      alert(typeof e === 'string' ? e : (e?.message || '新建文件夹失败'))
      return
    }
    await doMove(name)
  }

  const handleCreateFolder = async () => {
    const name = newFolder.trim()
    if (!name || folders.includes(name)) return
    try {
      await createAttachmentFolder(name)
      setActiveFolder(name)
      setNewFolder('')
      setAddingFolder(false)
    } catch (e: any) {
      console.error('新建文件夹失败', e)
      alert(typeof e === 'string' ? e : (e?.message || '新建文件夹失败'))
    }
  }

  const commitRenameFolder = async () => {
    if (!renameFolder) return
    const name = renameVal.trim()
    if (name && name !== renameFolder) {
      await renameAttachmentFolder(renameFolder, name)
    }
    setRenameFolder(null)
    setRenameVal('')
  }

  const handleDeleteFolder = (name: string) => {
    setDelFolder(name)
  }

  const handleOpenFolder = async (id: number) => {
    try {
      await openAttachmentFolder(id)
    } catch (e) {
      console.error('打开文件夹失败', e)
    }
  }

  const handleUpload1 = async (id: number) => {
    setUploadingIds((s) => new Set(s).add(id))
    try {
      const msg = await uploadAttachment(id)
      alert(msg)
    } catch (e: any) {
      alert(typeof e === 'string' ? e : (e?.message || '上传失败'))
    } finally {
      setUploadingIds((s) => { const n = new Set(s); n.delete(id); return n })
    }
  }

  const toggleSelect = (id: number) => {
    setSelected((s) => { const n = new Set(s); n.has(id) ? n.delete(id) : n.add(id); return n })
  }

  const allSelected = filtered.length > 0 && filtered.every((a) => selected.has(a.id))
  const toggleSelectAll = () => {
    if (allSelected) {
      setSelected(new Set())
    } else {
      setSelected(new Set(filtered.map((a) => a.id)))
    }
  }

  const exitSelectMode = () => {
    setSelectMode(false)
    setSelected(new Set())
  }

  const selectedIds = () => Array.from(selected)

  const handleBatchDelete = async () => {
    const ids = selectedIds()
    if (ids.length === 0) return
    setDeleteModal({ ids, single: false })
  }

  const handleBatchMove = () => {
    const ids = selectedIds()
    if (ids.length === 0) return
    setMovePicker({ ids })
  }

  const handleBatchUpload = async () => {
    const ids = selectedIds()
    if (ids.length === 0) return
    setBatchBusy(true)
    let ok = 0, failed = 0
    for (const id of ids) {
      try { await uploadAttachment(id); ok++ } catch { failed++ }
    }
    setBatchBusy(false)
    alert(`上传完成：成功 ${ok} 个${failed > 0 ? `，失败 ${failed} 个` : ''}`)
  }

  const handleBatchLink = () => {
    const ids = selectedIds()
    if (ids.length === 0) return
    setLinkModalAttId(-1)
  }

  const handleDownload = async (filename: string) => {
    setDownloadingNames((s) => new Set(s).add(filename))
    // 若当前选中了某个文件夹（非全部/未分类根目录），下载到该文件夹
    const dlFolder = activeFolder === '' || activeFolder === ACTIVE_ALL ? '' : activeFolder
    try {
      await downloadAttachment(filename, dlFolder)
      alert(`「${filename}」已下载${dlFolder ? `到「${dlFolder}」` : ''}`)
      await loadRemote()
    } catch (e: any) {
      alert(typeof e === 'string' ? e : (e?.message || '下载失败'))
    } finally {
      setDownloadingNames((s) => { const n = new Set(s); n.delete(filename); return n })
    }
  }

  const currentAtt = linkModalAttId !== null && linkModalAttId >= 0 ? attachments.find((a) => a.id === linkModalAttId) : null
  const batchLinkMode = linkModalAttId === -1
  const sortedNotes = useMemo(() => {
    return [...notes].sort((a, b) => b.id - a.id)
  }, [notes])
  const filteredNotes = linkSearch
    ? sortedNotes.filter((n) => n.text.includes(linkSearch))
    : sortedNotes

  const handleToggleLink = async (noteId: number) => {
    if (batchLinkMode) {
      for (const id of selectedIds()) {
        await handleLink(id, noteId)
      }
      return
    }
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
        <button className="btn btn-ghost" style={{ padding: '6px 12px', fontSize: '.74rem' }} onClick={handleDebug} disabled={debugLoading} title="返回 PROPFIND 目标地址、HTTP 状态与原始 XML，用于排查拿不到云端附件的问题">
          <i className="fas fa-bug"></i> {debugLoading ? '诊断中...' : '调试信息'}
        </button>
      </div>
      {remoteAtts !== null && (
        <div className="cloud-att-list">
          <div className="cloud-att-head">
            <span><i className="fas fa-cloud" style={{ color: 'var(--accent)', marginRight: 6 }}></i>云端附件（{remoteAtts.length}）</span>
            <div style={{ display: 'flex', gap: 6 }}>
              <button className="btn btn-ghost" style={{ padding: '2px 8px', fontSize: '.72rem' }} title="校验云端附件是否为加密上传" onClick={handleVerifyEncryption} disabled={verifyLoading}>
                <i className="fas fa-shield-alt"></i> {verifyLoading ? '校验中...' : '验证加密'}
              </button>
              <button className="btn btn-ghost" style={{ padding: '2px 8px', fontSize: '.72rem' }} onClick={loadRemote} disabled={remoteLoading}>
                <i className="fas fa-sync"></i> 刷新
              </button>
            </div>
          </div>
          {verifyResult && (
            <div className="em" style={{ color: verifyResult.includes('✅') ? 'var(--accent)' : 'var(--priority-high)', fontSize: '.72rem' }}>
              {verifyResult}
            </div>
          )}
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
              <button
                className="btn btn-secondary cloud-att-dl"
                title="下载到本地"
                disabled={downloadingNames.has(a.filename)}
                onClick={() => handleDownload(a.filename)}
              >
                <i className={'fas ' + (downloadingNames.has(a.filename) ? 'fa-spinner fa-spin' : 'fa-download')}></i>
              </button>
            </div>
          ))}
        </div>
      )}
      {debugInfo && (
        <div className="cloud-att-list" style={{ marginTop: 8 }}>
          <div className="cloud-att-head">
            <span><i className="fas fa-bug" style={{ color: 'var(--priority-high)', marginRight: 6 }}></i>诊断信息（PROPFIND 原始返回）</span>
            <button className="btn btn-ghost" style={{ padding: '2px 8px', fontSize: '.72rem' }} onClick={() => setDebugInfo('')}>关闭</button>
          </div>
          <pre style={{ whiteSpace: 'pre-wrap', wordBreak: 'break-all', fontSize: '.68rem', color: 'var(--text-secondary)', maxHeight: 320, overflow: 'auto', margin: 0 }}>{debugInfo}</pre>
        </div>
      )}
      <div className="file-toolbar">
        <button
          className={'btn ' + (selectMode ? 'btn-primary' : 'btn-secondary')}
          style={{padding:'4px 10px',fontSize:'.72rem'}}
          title="多选"
          onClick={() => selectMode ? exitSelectMode() : setSelectMode(true)}
        >
          <i className="fas fa-check-square"></i> {selectMode ? '退出多选' : '多选'}
        </button>
        <div className="file-search-box">
          <i className="fas fa-search"></i>
          <input
            value={fileSearch}
            onChange={(e) => setFileSearch(e.target.value)}
            placeholder="搜索资料名..."
          />
          {fileSearch && (
            <i className="fas fa-times file-search-clear" onClick={() => setFileSearch('')}></i>
          )}
        </div>
        <span className="file-cur-label">
          {activeFolder === ACTIVE_ALL ? `全部（${attachments.length}）` : activeFolder === '' ? `未分类（${folderCounts[''] || 0}）` : `${activeFolder}（${folderCounts[activeFolder] || 0}）`}
        </span>
      </div>
      <div className="file-folder-bar">
        <div className="file-folders">
          <span
            className={'file-capsule' + (activeFolder === ACTIVE_ALL ? ' active' : '')}
            onClick={() => setActiveFolder(ACTIVE_ALL)}
          >全部 ({attachments.length})</span>
          <span
            className={'file-capsule' + (activeFolder === '' ? ' active' : '')}
            onClick={() => setActiveFolder('')}
          ><span className="color-dot" style={{ background: '#9aa0aa' }}></span>未分类 ({folderCounts[''] || 0})</span>
          {folders.map((f) => (
            <span
              key={f}
              className={'file-capsule' + (activeFolder === f ? ' active' : '')}
              onClick={() => setActiveFolder(f)}
            ><span className="color-dot" style={{ background: folderColor(f) }}></span>{f} ({folderCounts[f] || 0})</span>
          ))}
          {addingFolder ? (
            <span className="file-capsule-input-wrap">
              <input
                autoFocus
                className="file-capsule-input"
                value={newFolder}
                onChange={(e) => setNewFolder(e.target.value)}
                onKeyDown={(e) => { if (e.key === 'Enter') handleCreateFolder(); if (e.key === 'Escape') setAddingFolder(false) }}
                placeholder="文件夹名"
              />
              <button className="btn btn-primary" style={{padding:'3px 8px',fontSize:'.66rem'}} onClick={handleCreateFolder}><i className="fas fa-check"></i></button>
            </span>
          ) : (
            <span className="file-capsule capsule-add" onClick={() => setAddingFolder(true)}><i className="fas fa-plus"></i> 新建</span>
          )}
          <span className={'file-capsule' + (manageOpen ? ' active' : '')} onClick={() => setManageOpen((v) => !v)}>
            <i className="fas fa-sliders-h"></i> 管理
          </span>
        </div>
      </div>
      {manageOpen && (
        <div className="file-manage-panel">
          <div className="file-manage-head">
            <span><i className="fas fa-folder-gear"></i> 文件夹管理</span>
            <span className="file-manage-hint">未分类资料默认进收件箱，可整理到文件夹</span>
          </div>
          <div className="file-manage-row">
            <span className="manage-dot" style={{ background: '#9aa0aa' }}></span>
            <span className="manage-name">未分类</span>
            <span className="manage-count">{folderCounts[''] || 0}</span>
            <button className="btn btn-secondary m-btn" title="打开文件夹" onClick={() => openAttachmentFolderByName('')}><i className="fas fa-folder-open"></i></button>
          </div>
          {folders.map((f) => (
            <div className="file-manage-row" key={f}>
              <span className="manage-dot" style={{ background: folderColor(f) }}></span>
              {renameFolder === f ? (
                <>
                  <input
                    autoFocus
                    className="file-capsule-input"
                    value={renameVal}
                    onChange={(e) => setRenameVal(e.target.value)}
                    onKeyDown={(e) => { if (e.key === 'Enter') commitRenameFolder(); if (e.key === 'Escape') { setRenameFolder(null); setRenameVal('') } }}
                    style={{ flex: 1, minWidth: 0 }}
                  />
                  <button className="btn btn-primary m-btn" onClick={commitRenameFolder}><i className="fas fa-check"></i></button>
                </>
              ) : (
                <>
                  <span className="manage-name">{f}</span>
                  <span className="manage-count">{folderCounts[f] || 0}</span>
                  <button className="btn btn-secondary m-btn" title="打开文件夹" onClick={() => openAttachmentFolderByName(f)}><i className="fas fa-folder-open"></i></button>
                  <button className="btn btn-secondary m-btn" onClick={() => { setRenameFolder(f); setRenameVal(f) }}>重命名</button>
                  <button className="btn btn-secondary m-btn" onClick={() => handleDeleteFolder(f)}>删除</button>
                </>
              )}
            </div>
          ))}
        </div>
      )}
      {selectMode && (
        <div className="file-batch-bar">
          <label className="file-batch-all" onClick={toggleSelectAll}>
            <i className={'far ' + (allSelected ? 'fa-check-square' : 'fa-square')}></i> 全选
          </label>
          <span className="file-batch-count">已选 {selected.size} 项</span>
          <div className="file-batch-actions">
            <button className="btn btn-secondary" disabled={selected.size === 0 || batchBusy} onClick={handleBatchMove}>
              <i className="fas fa-arrow-right"></i> 迁移
            </button>
            <button className="btn btn-secondary" disabled={selected.size === 0 || batchBusy} onClick={handleBatchLink}>
              <i className="fas fa-link"></i> 关联
            </button>
            {webdavConfigured && (
              <button className="btn btn-secondary" disabled={selected.size === 0 || batchBusy} onClick={handleBatchUpload}>
                <i className="fas fa-cloud-upload-alt"></i> {batchBusy ? '处理中...' : '上传'}
              </button>
            )}
            <button className="btn btn-secondary" disabled={selected.size === 0 || batchBusy} onClick={handleBatchDelete}>
              <i className="fas fa-trash"></i> 删除
            </button>
          </div>
        </div>
      )}
      {filtered.length === 0 ? (
        <div className="em">
          {fileSearch.trim() ? `未找到匹配「${fileSearch.trim()}」的资料`
            : activeFolder === '' ? '「未分类」暂无资料，新上传的资料会出现在这里'
            : activeFolder === ACTIVE_ALL ? '暂无资料'
            : (
              <>
                「{activeFolder}」暂无资料
                <div className="row">
                  <button className="btn btn-secondary" onClick={() => alert('请使用顶部「选择文件上传」或拖拽文件到此')}>＋ 上传资料</button>
                  <button className="btn btn-ghost" onClick={() => handleDeleteFolder(activeFolder)}>删除此文件夹</button>
                </div>
              </>
            )}
        </div>
      ) : (
        <div className="file-list">
          {filtered.map((a) => {
            const missing = missingFiles.has(a.id)
            return (
            <div className={'file-item' + (missing ? ' file-missing' : '') + (selectMode && selected.has(a.id) ? ' selected' : '')} key={a.id} onClick={selectMode ? () => toggleSelect(a.id) : undefined} style={selectMode ? { cursor: 'pointer' } : undefined}>
              {selectMode && (
                <div className="file-select-box">
                  <i className={'far ' + (selected.has(a.id) ? 'fa-check-square' : 'fa-square')}></i>
                </div>
              )}
              <div className="file-icon"><i className="fas fa-file"></i></div>
              <div className="file-info">
                <div className="file-name">{a.filename}</div>
                <div className="file-meta">
                  <span>{formatSize(a.size)}</span>
                  <span>{a.created}</span>
                  {a.folder
                    ? <span><span className="color-dot" style={{ background: folderColor(a.folder) }}></span> {a.folder}</span>
                    : <span><span className="color-dot" style={{ background: '#9aa0aa' }}></span> 未分类</span>}
                  {missing && <span className="file-missing-badge"><i className="fas fa-exclamation-triangle"></i> 文件已丢失</span>}
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
              {!selectMode && (
              <div className="file-actions">
                <button className="btn btn-secondary" title={missing ? '文件已丢失' : '打开所在文件夹'} disabled={missing} onClick={() => handleOpenFolder(a.id)}>
                  <i className="fas fa-folder-open"></i>
                </button>
                {webdavConfigured && (
                  <button className="btn btn-secondary" title={missing ? '文件已丢失' : '上传到云端'} disabled={missing || uploadingIds.has(a.id)} onClick={() => handleUpload1(a.id)}>
                    <i className={'fas ' + (uploadingIds.has(a.id) ? 'fa-spinner fa-spin' : 'fa-cloud-upload-alt')}></i>
                  </button>
                )}
                <button className="btn btn-secondary" title="关联笔记" onClick={() => setLinkModalAttId(a.id)}>
                  <i className="fas fa-link"></i>
                </button>
                <button className="btn btn-secondary" onClick={() => setMovePicker({ ids: [a.id] })}>
                  <i className="fas fa-arrow-right"></i>
                </button>
                <button className="btn btn-secondary" onClick={() => handleDelete(a.id)}>
                  <i className="fas fa-trash"></i>
                </button>
              </div>
              )}
            </div>
            )
          })}
        </div>
      )}

      {linkModalAttId !== null && (currentAtt || batchLinkMode) && (
        <div className="mo op" onClick={() => setLinkModalAttId(null)}>
          <div className="mb" onClick={(e) => e.stopPropagation()} style={{maxHeight:'80vh',display:'flex',flexDirection:'column'}}>
            <div style={{display:'flex',justifyContent:'space-between',alignItems:'center',marginBottom:'10px'}}>
              <strong style={{fontSize:'.9rem',color:'var(--text-primary)'}}>关联笔记</strong>
              <span style={{fontSize:'.75rem',color:'var(--text-muted)'}}>{batchLinkMode ? `批量关联 ${selected.size} 个资料` : `${currentAtt!.filename} · 已关联 ${currentAtt!.note_ids.length} 条`}</span>
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
                  const linked = batchLinkMode ? false : currentAtt!.note_ids.includes(n.id)
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

      {deleteModal && (
        <div className="mo op" onClick={() => setDeleteModal(null)}>
          <div className="mb" onClick={(e) => e.stopPropagation()} style={{maxWidth:'420px'}}>
            <div style={{display:'flex',alignItems:'center',gap:'8px',marginBottom:'12px'}}>
              <i className="fas fa-exclamation-triangle" style={{color:'var(--priority-high)',fontSize:'1.1rem'}}></i>
              <strong style={{fontSize:'.95rem',color:'var(--text-primary)'}}>确认删除</strong>
            </div>
            <div style={{fontSize:'.8rem',color:'var(--text-secondary)',lineHeight:1.6,marginBottom:'6px'}}>
              确定要删除{deleteModal.ids.length > 1 ? `选中的 ${deleteModal.ids.length} 个资料` : '该资料'}吗？此操作不可恢复（将同时删除本地文件{deleteModal.ids.length > 1 ? '及记录' : '与记录'}）。
            </div>
            {deleteModal.ids.length <= 5 && (
              <ul style={{margin:'4px 0 12px',paddingLeft:'18px',fontSize:'.74rem',color:'var(--text-muted)'}}>
                {deleteModal.ids.map((id) => {
                  const att = attachments.find((a) => a.id === id)
                  return <li key={id} style={{overflow:'hidden',textOverflow:'ellipsis',whiteSpace:'nowrap'}}>{att ? att.filename : `#${id}`}</li>
                })}
              </ul>
            )}
            <div style={{display:'flex',justifyContent:'flex-end',gap:'8px',marginTop:'8px'}}>
              <button className="btn btn-ghost" onClick={() => setDeleteModal(null)}>取消</button>
              <button className="btn btn-primary" style={{background:'var(--priority-high)',borderColor:'var(--priority-high)'}} onClick={confirmDelete}>
                <i className="fas fa-trash"></i> 删除
              </button>
            </div>
          </div>
        </div>
      )}

      {movePicker && (
        <div className="mo op" onClick={() => setMovePicker(null)}>
          <div className="mb" onClick={(e) => e.stopPropagation()} style={{maxWidth:'420px'}}>
            <div style={{display:'flex',alignItems:'center',gap:'8px',marginBottom:'10px'}}>
              <i className="fas fa-arrow-right" style={{color:'var(--accent)',fontSize:'1rem'}}></i>
              <strong style={{fontSize:'.95rem',color:'var(--text-primary)'}}>移动到文件夹</strong>
            </div>
            <div className="folder-picker-list">
              <div className="picker-item" onClick={() => doMove('')}>
                <span className="color-dot" style={{ background: '#9aa0aa' }}></span> 未分类（收件箱）
                <span className="num">{folderCounts[''] || 0}</span>
              </div>
              {folders.map((f) => (
                <div className="picker-item" key={f} onClick={() => doMove(f)}>
                  <span className="color-dot" style={{ background: folderColor(f) }}></span> {f}
                  <span className="num">{folderCounts[f] || 0}</span>
                </div>
              ))}
            </div>
            <div className="picker-new">
              <input
                id="pickerNew"
                placeholder="新建文件夹..."
                onKeyDown={async (e) => { if (e.key === 'Enter') { const v = (e.target as HTMLInputElement).value.trim(); if (v && !folders.includes(v)) await createAndMove(v) } }}
              />
              <button className="btn btn-primary" onClick={async () => {
                const el = document.getElementById('pickerNew') as HTMLInputElement | null
                const v = el?.value.trim() || ''
                if (v && !folders.includes(v)) await createAndMove(v)
              }}>新建并移动</button>
            </div>
            <div style={{display:'flex',justifyContent:'flex-end',marginTop:'10px'}}>
              <button className="btn btn-ghost" onClick={() => setMovePicker(null)}>取消</button>
            </div>
          </div>
        </div>
      )}

      {delFolder && (
        <div className="mo op" onClick={() => setDelFolder(null)}>
          <div className="mb" onClick={(e) => e.stopPropagation()} style={{maxWidth:'420px'}}>
            <div style={{display:'flex',alignItems:'center',gap:'8px',marginBottom:'10px'}}>
              <i className="fas fa-exclamation-triangle" style={{color:'var(--priority-high)',fontSize:'1.1rem'}}></i>
              <strong style={{fontSize:'.95rem',color:'var(--text-primary)'}}>删除文件夹「{delFolder}」</strong>
            </div>
            <p style={{fontSize:'.8rem',color:'var(--text-secondary)',lineHeight:1.6,margin:'0 0 12px'}}>
              该文件夹内有 <b>{folderCounts[delFolder] || 0}</b> 个资料，请选择如何处理：
            </p>
            <div style={{display:'flex',justifyContent:'flex-end',gap:'8px'}}>
              <button className="btn btn-ghost" onClick={() => setDelFolder(null)}>取消</button>
              <button className="btn btn-secondary" onClick={() => { deleteAttachmentFolder(delFolder, 'move_root'); setDelFolder(null); setActiveFolder(ACTIVE_ALL) }}>
                资料移回未分类
              </button>
              <button className="btn btn-primary" style={{background:'var(--priority-high)',borderColor:'var(--priority-high)'}} onClick={() => { deleteAttachmentFolder(delFolder, 'delete'); setDelFolder(null); setActiveFolder(ACTIVE_ALL) }}>
                连同资料删除
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
