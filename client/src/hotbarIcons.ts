/** Hotbar icons: the inside of a 24×24 SVG per item id, drawn in `currentColor` plus a few accents. */
export const HOTBAR_ICONS: Record<number, string> = {
  // Revolver
  1: '<path d="M3 8h13l2 2v2h-6l-1 5H8l1-5H3z" fill="currentColor"/><circle cx="12" cy="9" r="1.4" fill="#fff" opacity=".6"/>',
  // AK5
  2: '<path d="M1 11h10l2-2h6l1 2h3v2h-5l-1 2h-3l-1-2H11l-2 5H6l1-5H1z" fill="currentColor"/>',
  // Grenade
  3: '<rect x="9" y="2" width="6" height="3" rx="1" fill="currentColor"/><path d="M15 3l4-1" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/><ellipse cx="12" cy="14" rx="6.5" ry="7.5" fill="#5b7a3a"/><path d="M7 11h10M7 15h10M12 7v14" stroke="#2f4320" stroke-width="1"/>',
  // Bandage
  4: '<rect x="2" y="8" width="20" height="8" rx="4" fill="#f4e6c8" stroke="currentColor" stroke-width="1.5" transform="rotate(-30 12 12)"/><path d="M10 9.5v5M14 9.5v5" stroke="#d94f4f" stroke-width="2" transform="rotate(-30 12 12)"/>',
  // Sniper
  5: '<path d="M0 12h14l1-1h8v2h-8l-1 1H9l-1 4H5l1-4H0z" fill="currentColor"/><rect x="7" y="7" width="8" height="2.5" rx="1" fill="#4a90d9"/>',
  // Golden egg
  6: '<ellipse cx="12" cy="13" rx="6.5" ry="8.5" fill="#ffbe0b" stroke="#b78400" stroke-width="1.5"/><ellipse cx="9.5" cy="9.5" rx="1.6" ry="2.6" fill="#fff" opacity=".6"/>',
  // Shotgun
  7: '<path d="M0 10h17l2 1.5h5V14h-5l-2 1H11l-2 5H6l1-5H0z" fill="currentColor"/><rect x="4" y="12" width="7" height="2" fill="#8a5a2b"/>',
  // FPV drone
  8: '<circle cx="5" cy="6" r="3" fill="none" stroke="currentColor" stroke-width="1.5"/><circle cx="19" cy="6" r="3" fill="none" stroke="currentColor" stroke-width="1.5"/><circle cx="5" cy="18" r="3" fill="none" stroke="currentColor" stroke-width="1.5"/><circle cx="19" cy="18" r="3" fill="none" stroke="currentColor" stroke-width="1.5"/><path d="M7 8l10 8M17 8L7 16" stroke="currentColor" stroke-width="2"/><rect x="9.5" y="9.5" width="5" height="5" rx="1.5" fill="currentColor"/>',
  // Loitering drone
  9: '<path d="M2 14l10-4 10 4-10 2z" fill="currentColor"/><path d="M12 10V4M8 5h8" stroke="currentColor" stroke-width="2" stroke-linecap="round"/><path d="M9 17l3 5 3-5" fill="#d94f4f"/>',
  // Flare gun
  10: '<path d="M2 9h12l2 2v2h-5l-1 5H7l1-5H2z" fill="#d94f4f"/><path d="M18 5l1 3 3 1-3 1-1 3-1-3-3-1 3-1z" fill="#ffbe0b"/>',
  // Bricks
  11: '<rect x="2" y="5" width="9" height="6" fill="#c0603a"/><rect x="13" y="5" width="9" height="6" fill="#c0603a"/><rect x="7" y="13" width="10" height="6" fill="#c0603a"/><path d="M2 5h20v14H2zM11 5v6M7 13h10" fill="none" stroke="#fff" stroke-width="1" opacity=".5"/>',
  // Carl Gustaf
  12: '<rect x="1" y="9" width="22" height="5" rx="2" fill="#55613a"/><path d="M1 8.5h4v6H1zM19 8.5h4v6h-4z" fill="#3a4426"/><path d="M9 14v5h3v-5M13 5h4v4h-4z" fill="currentColor"/>',
}
