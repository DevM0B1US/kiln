/// Inline Lucide marks. The contract says Lucide for every glyph; inline
/// SVGs are the web-native way to keep that, independent of font fallback.
/// Each constant is the inner body of a 24x24 viewBox SVG.

export const MINUS = '<path d="M5 12h14"/>';
export const SQUARE = '<rect x="5" y="5" width="14" height="14" rx="1"/>';
export const RESTORE =
  '<rect x="3" y="8" width="13" height="13" rx="1"/><path d="M8 8V5a2 2 0 0 1 2-2h9a1 1 0 0 1 1 1v9a2 2 0 0 1-2 2h-3"/>';
export const CLOSE = '<path d="M18 6 6 18M6 6l12 12"/>';
export const HEXAGON =
  '<path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"/>';
export const PLUS = '<path d="M5 12h14M12 5v14"/>';
export const CHECKER =
  '<rect x="3" y="3" width="7" height="7"/><rect x="14" y="14" width="7" height="7"/><rect x="14" y="3" width="7" height="7" stroke-dasharray="2 2"/><rect x="3" y="14" width="7" height="7" stroke-dasharray="2 2"/>';
export const FILLED_DARK =
  '<rect x="4" y="4" width="16" height="16" fill="currentColor" stroke="none"/>';
export const SQUARE_WHITE = '<rect x="4" y="4" width="16" height="16"/>';
export const CHEVRONS_LEFT = '<path d="m11 17-5-5 5-5m7 10-5-5 5-5"/>';
export const CHEVRONS_RIGHT = '<path d="m6 17 5-5-5-5m7 10 5-5-5-5"/>';
