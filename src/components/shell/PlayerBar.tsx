import { SpotIcon } from "../icons/SpotIcon";

export function PlayerBar() {
  return (
    <footer className="player-bar" aria-label="Now playing">
      <div className="player-track-empty">
        <div className="player-art-placeholder"><SpotIcon name="play" size={20} /></div>
        <div>
          <span className="player-empty-label">Nothing queued</span>
          <span className="player-empty-caption">Choose a track to start listening</span>
        </div>
      </div>
      <div className="player-controls">
        <button aria-label="Previous track" className="player-icon-button" disabled type="button"><SpotIcon name="previous" size={18} /></button>
        <button aria-label="Play" className="player-play-button" disabled type="button"><SpotIcon name="play" size={17} /></button>
        <button aria-label="Next track" className="player-icon-button" disabled type="button"><SpotIcon name="next" size={18} /></button>
      </div>
      <div className="player-right">
        <span className="player-source-label">SOURCE <strong>—</strong></span>
        <span className="player-expand player-expand-disabled">Player view unlocks when a track is queued</span>
      </div>
    </footer>
  );
}
