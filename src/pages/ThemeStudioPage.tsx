import { ThemeStudio } from "../features/theme/ThemeStudio";

export function ThemeStudioPage() {
  return <div className="page-stack theme-studio-page"><section className="page-intro"><div><span className="eyebrow">EXPLORE / THEME STUDIO</span><h1>Make a place to <em>listen.</em></h1><p>Set the colors SpotDIY uses throughout the app, then save the palette when it feels right.</p></div><div className="page-intro-stat"><strong>15</strong><span>semantic tokens</span></div></section><ThemeStudio /></div>;
}
