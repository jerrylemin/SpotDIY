import { ThemeStudio } from "../features/theme/ThemeStudio";

export function ThemeStudioPage() {
  return <div className="page-stack theme-studio-page"><section className="page-intro"><div><span className="eyebrow">EXPLORE / THEME STUDIO</span><h1>Make a place to <em>listen.</em></h1><p>Draft semantic colors, preview them for this session, then save only when they feel right.</p></div><div className="page-intro-stat"><strong>15</strong><span>semantic tokens</span></div></section><ThemeStudio /></div>;
}
