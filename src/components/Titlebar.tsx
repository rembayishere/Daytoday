export default function Titlebar() {
  return (
    <div className="titlebar">
      <div className="titlebar-left">
        <div className="dot red"></div>
        <div className="dot yellow"></div>
        <div className="dot green"></div>
        <span className="app-title">Daytoday</span>
      </div>
      <div className="tray-icon"><i className="fas fa-tray"></i></div>
    </div>
  )
}
