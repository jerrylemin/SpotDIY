import { SpotIcon } from "./SpotIcon";
import { spotIconNames } from "./spot-icon-data";

export function IconGallery() {
  return (
    <section aria-label="SpotDIY icon gallery" className="icon-gallery">
      <div className="icon-gallery-grid">
        {spotIconNames.map((name) => (
          <div className="icon-gallery-item" key={name}>
            <span className="icon-gallery-mark"><SpotIcon name={name} size={24} /></span>
            <span>{name}</span>
          </div>
        ))}
      </div>
    </section>
  );
}
