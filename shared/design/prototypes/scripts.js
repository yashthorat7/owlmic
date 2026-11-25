(() => {
  const $ = id => document.getElementById(id);
  const $$ = sel => document.querySelectorAll(sel);

  // Inline monochrome cubic SVG fallbacks with safe URI encoding & resolution cache
  const FALLBACK_SVG = {
  owl: `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100" fill="#000"><path fill-rule="evenodd" d="M12 20h20v20H12zm24 0h28v20H36zm32 0h20v20H68zm-44 24h20v20H24zm28 0h24v20H52zm-32 24h20v20H20zm24 0h16v20H44zm20 0h16v20H64z"/></svg>`,
  camera: `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="1.7" stroke-linecap="butt" stroke-linejoin="round"><path d="M 2.5 17.3 V 8.7 A 2.2 2.2 0 0 1 4.7 6.5 H 6.7 A 1.1 1.1 0 0 0 7.8 5.4 V 5.1 A 1.1 1.1 0 0 1 8.9 4 H 15.1 A 1.1 1.1 0 0 1 16.2 5.1 V 5.4 A 1.1 1.1 0 0 0 17.3 6.5 H 19.3 A 2.2 2.2 0 0 1 21.5 8.7 V 17.3 A 2.2 2.2 0 0 1 19.3 19.5 H 4.7 A 2.2 2.2 0 0 1 2.5 17.3 Z"/><rect x="9" y="10" width="6" height="6" rx="1.8"/></svg>`,
  'camera-off': `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="1.7" stroke-linecap="butt" stroke-linejoin="round"><path d="M 8.12 4.32 A 1.1 1.1 0 0 1 8.9 4 H 15.1 A 1.1 1.1 0 0 1 16.2 5.1 V 5.4 A 1.1 1.1 0 0 0 17.3 6.5 H 19.3 A 2.2 2.2 0 0 1 21.5 8.7 V 17.3 A 2.2 2.2 0 0 1 21.39 18.0"/><path d="M 16.4 19.5 H 4.7 A 2.2 2.2 0 0 1 2.5 17.3 V 8.7 A 2.2 2.2 0 0 1 3.2 7.09"/><path d="M 12.6 10 H 13.2 A 1.8 1.8 0 0 1 15 11.8 V 12.4"/><path d="M 9 11.8 V 14.2 A 1.8 1.8 0 0 0 10.8 16 H 13.2"/><path d="M 3.5 3.5 L 20.5 20.5"/></svg>`,
  mic: `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="1.7" stroke-linecap="butt" stroke-linejoin="round"><rect x="8.75" y="3" width="6.5" height="10.5" rx="2.2"/><path d="M 5.5 10 V 14.25 A 2.5 2.5 0 0 0 8 16.75 H 16 A 2.5 2.5 0 0 0 18.5 14.25 V 10"/><path d="M 12 16.75 V 20.5"/><path d="M 8 20.5 H 16"/></svg>`,
  'mic-off': `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="1.7" stroke-linecap="butt" stroke-linejoin="round"><path d="M 8.75 6 V 5.2 A 2.2 2.2 0 0 1 10.95 3 H 13.05 A 2.2 2.2 0 0 1 15.25 5.2 V 11.3"/><path d="M 8.78 11.68 A 2.2 2.2 0 0 0 10.57 13.47"/><path d="M 5.5 10 V 14.25 A 2.5 2.5 0 0 0 8 16.75 H 13.6"/><path d="M 18.5 10 V 14.25 A 2.5 2.5 0 0 1 18.38 15"/><path d="M 12 16.75 V 20.5"/><path d="M 8 20.5 H 16"/><path d="M 3.5 3.5 L 20.5 20.5"/></svg>`,
  speaker: `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="1.7" stroke-linecap="butt" stroke-linejoin="round"><path d="M 3.9 8.75 H 6.5 L 10.6 5.25 C 11.15 4.8 11.5 5 11.5 5.7 V 18.3 C 11.5 19 11.15 19.2 10.6 18.75 L 6.5 15.25 H 3.9 A 1.4 1.4 0 0 1 2.5 13.85 V 10.15 A 1.4 1.4 0 0 1 3.9 8.75 Z"/><path d="M 14.75 8.75 L 16.25 10.15 V 13.85 L 14.75 15.25"/><path d="M 17.75 6 L 20.25 8.5 V 15.5 L 17.75 18"/></svg>`,
  'speaker-off': `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="1.7" stroke-linecap="butt" stroke-linejoin="round"><path d="M 3.9 8.75 H 6.5 L 10.6 5.25 C 11.15 4.8 11.5 5 11.5 5.7 V 18.3 C 11.5 19 11.15 19.2 10.6 18.75 L 6.5 15.25 H 3.9 A 1.4 1.4 0 0 1 2.5 13.85 V 10.15 A 1.4 1.4 0 0 1 3.9 8.75 Z"/><path d="M 15 9.375 L 20.25 14.625"/><path d="M 20.25 9.375 L 15 14.625"/></svg>`,
  help: `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2.0" stroke-linecap="butt" stroke-linejoin="round"><path d="M 7.5 8.25 V 5.8 A 1.8 1.8 0 0 1 9.3 4 H 14.7 A 1.8 1.8 0 0 1 16.5 5.8 V 9.2 A 1.8 1.8 0 0 1 14.7 11 H 13.5 A 1.5 1.5 0 0 0 12 12.5 V 15"/><rect x="10.9" y="18.15" width="2.2" height="2.2" rx="0.4" fill="#000" stroke="none"/></svg>`,
  globe: `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2.0" stroke-linecap="butt" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="4.2"/><path d="M 10.8 3 A 2.3 2.3 0 0 0 8.5 5.3 V 18.7 A 2.3 2.3 0 0 0 10.8 21"/><path d="M 13.2 3 A 2.3 2.3 0 0 1 15.5 5.3 V 18.7 A 2.3 2.3 0 0 1 13.2 21"/><path d="M 3 12 H 21"/></svg>`,
  wifi: `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2.0" stroke-linecap="butt" stroke-linejoin="round"><path d="M 2.75 9.25 L 5.75 6.25 H 18.25 L 21.25 9.25"/><path d="M 5.75 12.25 L 8 10 H 16 L 18.25 12.25"/><path d="M 8.75 15.25 L 10.25 13.75 H 13.75 L 15.25 15.25"/><rect x="10.8" y="17.8" width="2.4" height="2.4" rx="0.4" fill="#000" stroke="none"/></svg>`,
  cable: `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2.0" stroke-linecap="butt" stroke-linejoin="round"><rect x="10.6" y="10.6" width="2.8" height="2.8" rx="0.5" fill="#000" stroke="none"/><path d="M 8.25 8.25 L 6.5 10 V 14 L 8.25 15.75"/><path d="M 5.5 5.5 L 3 8 V 16 L 5.5 18.5"/><path d="M 15.75 8.25 L 17.5 10 V 14 L 15.75 15.75"/><path d="M 18.5 5.5 L 21 8 V 16 L 18.5 18.5"/></svg>`,
  usb: `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2.0" stroke-linecap="butt" stroke-linejoin="round"><path d="M 12 4.25 V 18"/><path d="M 9.25 7 L 12 4.25 L 14.75 7"/><rect x="10.5" y="17.8" width="3" height="3" rx="0.5" fill="#000" stroke="none"/><path d="M 12 16 L 6.75 13.25 V 11.25"/><rect x="5.25" y="8.25" width="3" height="3" rx="0.5"/><path d="M 12 13.75 L 17.25 11.25 V 10.1"/><path d="M 17.25 6.4 L 19.1 8.25 L 17.25 10.1 L 15.4 8.25 Z"/></svg>`,
  bluetooth: `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2.0" stroke-linecap="butt" stroke-linejoin="round"><path d="M 6.5 8.46 L 17.5 16.25 L 11.5 20.5 V 3.5 L 17.5 7.75 L 6.5 15.54"/></svg>`,
  link: `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2.3" stroke-linecap="butt" stroke-linejoin="round"><path d="M 10 8 H 4.5 A 2 2 0 0 0 2.5 10 V 14 A 2 2 0 0 0 4.5 16 H 10"/><path d="M 14 8 H 19.5 A 2 2 0 0 1 21.5 10 V 14 A 2 2 0 0 1 19.5 16 H 14"/><path d="M 8 12 H 16"/></svg>`,
  'link-off': `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2.3" stroke-linecap="butt" stroke-linejoin="round"><path d="M 5.3 8 H 4.5 A 2 2 0 0 0 2.5 10 V 14 A 2 2 0 0 0 4.5 16 H 10"/><path d="M 14 8 H 19.5 A 2 2 0 0 1 21.5 10 V 14 A 2 2 0 0 1 19.5 16 H 18.7"/><path d="M 4 4 L 20 20"/></svg>`,
  mirror: `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2.3" stroke-linecap="butt" stroke-linejoin="round"><path d="M 8.5 4.5 H 5 A 2 2 0 0 0 3 6.5 V 17.5 A 2 2 0 0 0 5 19.5 H 8.5"/><path d="M 15.5 4.5 H 19 A 2 2 0 0 1 21 6.5 V 17.5 A 2 2 0 0 1 19 19.5 H 15.5"/><path d="M 12 4 V 6.5 M 12 10.75 V 13.25 M 12 17.5 V 20"/></svg>`,
  refresh: `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2.3" stroke-linecap="butt" stroke-linejoin="round"><path d="M 4 11 V 8.55 A 2.8 2.8 0 0 1 6.8 5.75 H 19.25"/><path d="M 16.5 3 L 19.25 5.75 L 16.5 8.5"/><path d="M 20 13 V 15.45 A 2.8 2.8 0 0 1 17.2 18.25 H 4.75"/><path d="M 7.5 15.5 L 4.75 18.25 L 7.5 21"/></svg>`,
  copy: `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2.45" stroke-linecap="butt" stroke-linejoin="round"><rect x="8.5" y="8.5" width="11.5" height="11.5" rx="1.8"/><path d="M 5.8 15.5 A 1.8 1.8 0 0 1 4 13.7 V 5.8 A 1.8 1.8 0 0 1 5.8 4 H 13.7 A 1.8 1.8 0 0 1 15.5 5.8"/></svg>`,
  check: `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2.45" stroke-linecap="butt" stroke-linejoin="round"><path d="M 4.5 12.5 L 9.5 17.5 L 19.5 6.5"/></svg>`,
  arrow: `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2.15" stroke-linecap="butt" stroke-linejoin="round"><path d="M 4 12 H 19"/><path d="M 12 6 H 13.8 L 19.5 12 L 13.8 18 H 12"/></svg>`,
  plus: `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2.15" stroke-linecap="butt" stroke-linejoin="round"><path d="M 12 4.5 V 19.5 M 4.5 12 H 19.5"/></svg>`,
  list: `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2.15" stroke-linecap="butt" stroke-linejoin="round"><rect x="3.5" y="5.375" width="2.25" height="2.25" rx="0.35" fill="#000" stroke="none"/><rect x="3.5" y="10.875" width="2.25" height="2.25" rx="0.35" fill="#000" stroke="none"/><rect x="3.5" y="16.375" width="2.25" height="2.25" rx="0.35" fill="#000" stroke="none"/><path d="M 9 6.5 H 20.5 M 9 12 H 20.5 M 9 17.5 H 20.5"/></svg>`,
  shield: `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2.15" stroke-linecap="butt" stroke-linejoin="round"><path d="M 6.1 3.5 H 17.9 Q 19.5 3.5 19.5 5.1 V 12.6 Q 19.5 13.6 18.7 14.35 L 12 20.5 L 5.3 14.35 Q 4.5 13.6 4.5 12.6 V 5.1 Q 4.5 3.5 6.1 3.5 Z"/></svg>`,
  star: `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2.15" stroke-linecap="butt" stroke-linejoin="round"><path d="M 12 3.5 L 14.47 9.1 L 20.56 9.72 L 16 13.8 L 17.29 19.78 L 12 16.7 L 6.71 19.78 L 8 13.8 L 3.44 9.72 L 9.53 9.1 Z"/></svg>`,
  github: `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2.15" stroke-linecap="butt" stroke-linejoin="round"><path d="M 15 20.5 V 16 Q 15 14.5 16.5 14.5 H 17.25 Q 19.5 14.5 19.5 12.25 V 7.4 L 18.1 3.75 L 15.1 5.4 H 8.9 L 5.9 3.75 L 4.5 7.4 V 12.25 Q 4.5 14.5 6.75 14.5 H 7.5 Q 9 14.5 9 16 V 20.5"/><path d="M 9 18.5 H 6.5 Q 5.25 18.5 5.25 17.25 Q 5.25 16.25 4.25 16.25 H 3.5"/></svg>`,
  coffee: `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2.15" stroke-linecap="butt" stroke-linejoin="round"><path d="M 3.5 8.5 H 16 V 17.3 A 2.2 2.2 0 0 1 13.8 19.5 H 5.7 A 2.2 2.2 0 0 1 3.5 17.3 Z"/><path d="M 16 10 H 18.6 A 1.4 1.4 0 0 1 20 11.4 V 14.1 A 1.4 1.4 0 0 1 18.6 15.5 H 16"/><path d="M 7.5 3.5 V 6 M 12 3.5 V 6"/></svg>`,
  users: `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2.15" stroke-linecap="butt" stroke-linejoin="round"><rect x="6" y="4" width="6.5" height="6.5" rx="1.8"/><path d="M 3 20 V 16.2 A 2.2 2.2 0 0 1 5.2 14 H 13.3 A 2.2 2.2 0 0 1 15.5 16.2 V 20"/><path d="M 15.75 4.75 H 17.25 A 1.6 1.6 0 0 1 18.85 6.35 V 8.65 A 1.6 1.6 0 0 1 17.25 10.25 H 15.75"/><path d="M 17.5 14 H 19 A 2 2 0 0 1 21 16 V 20"/></svg>`,
  'arrow-left': `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2.15" stroke-linecap="butt" stroke-linejoin="round" class="rotate-180"><path d="M 4 12 H 19"/><path d="M 12 6 H 13.8 L 19.5 12 L 13.8 18 H 12"/></svg>`,
  'arrow-right': `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2.15" stroke-linecap="butt" stroke-linejoin="round"><path d="M 4 12 H 19"/><path d="M 12 6 H 13.8 L 19.5 12 L 13.8 18 H 12"/></svg>`,
  external: `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2.45" stroke-linecap="butt" stroke-linejoin="round"><path d="M 6.5 17.5 L 17.5 6.5"/><path d="M 8.5 6.5 H 17.5 V 15.5"/></svg>`,
  close: `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2.3" stroke-linecap="butt" stroke-linejoin="round"><path d="M 6.5 6.5 L 17.5 17.5 M 17.5 6.5 L 6.5 17.5"/></svg>`,
  flip: `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2.3" stroke-linecap="butt" stroke-linejoin="round"><path d="M 4 11 V 8.55 A 2.8 2.8 0 0 1 6.8 5.75 H 19.25"/><path d="M 16.5 3 L 19.25 5.75 L 16.5 8.5"/><path d="M 20 13 V 15.45 A 2.8 2.8 0 0 1 17.2 18.25 H 4.75"/><path d="M 7.5 15.5 L 4.75 18.25 L 7.5 21"/></svg>`,
  eye: `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2.3" stroke-linecap="butt" stroke-linejoin="round"><path d="M 2.5 12 C 5.5 6.5 8.5 5 12 5 C 15.5 5 18.5 6.5 21.5 12 C 18.5 17.5 15.5 19 12 19 C 8.5 19 5.5 17.5 2.5 12 Z"/><rect x="9.5" y="9.5" width="5" height="5" rx="2.2"/></svg>`,
  'eye-off': `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2.3" stroke-linecap="butt" stroke-linejoin="round"><path d="M 10.2 5.2 C 10.8 5.05 11.4 5 12 5 C 15.5 5 18.5 6.5 21.5 12 C 20.4 14.0 19.2 15.5 17.9 16.6"/><path d="M 14.1 18.6 C 13.4 18.9 12.7 19 12 19 C 8.5 19 5.5 17.5 2.5 12 C 3.8 9.6 5.2 7.9 6.7 6.8"/><path d="M 11.4 9.6 A 2.5 2.5 0 0 1 14.4 12.6"/><path d="M 9.6 11.4 A 2.5 2.5 0 0 0 12.6 14.4"/><path d="M 3.5 3.5 L 20.5 20.5"/></svg>`,
  phone: `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2.15" stroke-linejoin="round"><rect x="6.5" y="2.5" width="11" height="19" rx="2.5"/><path d="M 10.5 18 L 13.5 18" stroke-linecap="round"/></svg>`,
  unplug: `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2.3" stroke-linecap="butt" stroke-linejoin="round"><path d="M 5.3 8 H 4.5 A 2 2 0 0 0 2.5 10 V 14 A 2 2 0 0 0 4.5 16 H 10"/><path d="M 14 8 H 19.5 A 2 2 0 0 1 21.5 10 V 14 A 2 2 0 0 1 19.5 16 H 18.7"/><path d="M 4 4 L 20 20"/></svg>`,
  'git-branch': `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2.15" stroke-linecap="butt" stroke-linejoin="round"><path d="M 15 20.5 V 16 Q 15 14.5 16.5 14.5 H 17.25 Q 19.5 14.5 19.5 12.25 V 7.4 L 18.1 3.75 L 15.1 5.4 H 8.9 L 5.9 3.75 L 4.5 7.4 V 12.25 Q 4.5 14.5 6.75 14.5 H 7.5 Q 9 14.5 9 16 V 20.5"/><path d="M 9 18.5 H 6.5 Q 5.25 18.5 5.25 17.25 Q 5.25 16.25 4.25 16.25 H 3.5"/></svg>`,
  };

  const resolvedSrc = {};
  const fallbackUri = key => `data:image/svg+xml;charset=utf-8,${encodeURIComponent(FALLBACK_SVG[key])}`;

  const bindFallback = img => {
    if (!img || img.dataset.fbBound) return;
    const key = img.dataset.brand || img.dataset.icon;
    if (!key) return;
    img.dataset.fbBound = '1';

    if (resolvedSrc[key]) {
      img.src = resolvedSrc[key];
      return;
    }

    const handleErr = function() {
      if (!this.dataset.triedAlt) {
        this.dataset.triedAlt = '1';
        const iconKey = (key === 'arrow-left' || key === 'arrow-right') ? 'arrow' : key;
        this.src = img.dataset.brand ? '../../assets/brand/icon.svg' : `../../assets/icons/${iconKey}.svg`;
      } else if (FALLBACK_SVG[key]) {
        this.onerror = null;
        const uri = fallbackUri(key);
        resolvedSrc[key] = uri;
        this.src = uri;
      }
    };

    img.onload = function() {
      if (this.naturalWidth > 0 && !resolvedSrc[key]) resolvedSrc[key] = this.src;
    };
    img.onerror = handleErr;

    if (img.complete && img.naturalWidth === 0) handleErr.call(img);
  };

  const bindAllFallbacks = (root = document) => root.querySelectorAll('img[data-icon], img[data-brand]').forEach(bindFallback);
  const iconImg = (name, cls, alt = '') => {
    let key = name.replace(/\.svg$/, '');
    let extraCls = '';
    if (key === 'arrow-left') {
      key = 'arrow';
      extraCls = ' rotate-180';
    } else if (key === 'arrow-right') {
      key = 'arrow';
    }
    const src = resolvedSrc[key] || `../../assets/icons/${key}.svg`;
    return `<img src="${src}" class="${cls}${extraCls}" alt="${alt}" data-icon="${key}" />`;
  };

  const CFG = {
    links: ['USB', 'Tethering', 'Wi-Fi', 'Bluetooth'],
    shortLinks: { USB: 'USB', Tethering: 'Tether', 'Wi-Fi': 'Wi-Fi', Bluetooth: 'BT' },
    icons: { USB: 'usb', Tethering: 'cable', 'Wi-Fi': 'wifi', Bluetooth: 'bluetooth', Disconnected: 'globe' },
    ratios: { Auto: 0, '16:9': 16/9, '4:3': 4/3, '2:1': 2, '1:1': 1, '1:2': 0.5, '3:4': 3/4, '9:16': 9/16 },
    help: [
      { k: 'howto', ic: 'list', t: 'How-to' }, { k: 'privacy', ic: 'shield', t: 'Privacy' },
      { href: 'https://github.com/diveshpatil9104/owlmic', ic: 'star', t: 'Rate' },
      { href: 'https://github.com/diveshpatil9104/owlmic', ic: 'github', t: 'GitHub' },
      { href: 'https://github.com/sponsors/diveshpatil9104', ic: 'coffee', t: 'Donate' },
      { k: 'credits', ic: 'users', t: 'Credits' }
    ],
    credits: [
      ['Divesh Patil', 'diveshpatil9104'], ['Yash Thorat', 'yashthorat7'], ['Vedant Newadkar', 'vedantnewadkar'],
      ['Vaibhav Sonawane', 'Vaibhav-Sonawane105'], ['aipd506', 'aipd506']
    ],
    bars: 25,
    hudIdleMs: 4000
  };

  // ===================== UNIFIED HIVE-MIND STATE =====================
  const makeDefaultState = () => ({
    booting: true,
    phoneTrusted: false,
    pcTrusted: false,
    pairingRequest: false,
    userDisconnected: false,
    lastConnectedAt: null,
    phoneBlocked: false,
    pcBlocked: false,
    camera: false,
    mic: false,
    speaker: false,
    lens: 'Front',
    quality: 'FHD',
    fps: '60',
    ratio: 'Auto',
    mirror: true,
    nr: 5,
    vol: 0,
    voiceIso: true,
    normalization: true,
    audioLevel: 0,

    sim: {
      phone: { USB: false, Tethering: false, 'Wi-Fi': true, Bluetooth: false },
      pc:    { USB: false, Tethering: false, 'Wi-Fi': true, Bluetooth: false }
    },

    phone: {
      powered: true, isLandscape: false,
      drawer: null, camHud: false, camMenu: false, camHidden: false, micQuick: false, dockOpen: false,
      links: { USB: true, Tethering: true, 'Wi-Fi': true, Bluetooth: true }
    },
    pc: {
      flyoutOpen: false,
      trayOverflowOpen: true,
      drawer: null, camHud: false, camMenu: false, camHidden: false, micQuick: false, dockOpen: false,
      links: { USB: true, Tethering: true, 'Wi-Fi': true, Bluetooth: true }
    }
  });

  const state = makeDefaultState();

  // ===================== BUILD REPEATED TEMPLATES =====================
  const buildSimLinks = (side) => CFG.links.map(k =>
    `<button data-sim-side="${side}" data-sim-link="${k}" aria-label="${side} ${k}" class="sim-link h-[36px] rounded-[8px] flex items-center justify-center gap-1.5 text-[11px] font-medium cursor-pointer">${iconImg(CFG.icons[k], 'w-[16px] h-[16px] owl-icon shrink-0', k)}<span>${CFG.shortLinks[k]}</span></button>`
  ).join('');
  $('s-phoneLinks').innerHTML = buildSimLinks('phone');
  $('s-pcLinks').innerHTML = buildSimLinks('pc');

  const buildCamBtns = (items, attr, cls, sz, fmt = v => v) => items.map(v =>
    `<button ${attr}="${v}" class="${cls} ${sz} flex items-center justify-center tabular-nums text-center whitespace-nowrap cursor-pointer transition-colors font-medium">${fmt(v)}</button>`
  ).join('');

  const ratioIsWide = () => {
    if (state.ratio === '16:9' || state.ratio === '4:3' || state.ratio === '2:1') return true;
    if (state.ratio === '9:16' || state.ratio === '3:4' || state.ratio === '1:2') return false;
    return state.phone.isLandscape;
  };
  let bindRatioBtns = () => {};
  const syncRatioMenus = () => {
    const wide = ratioIsWide();
    const row1 = wide ? ['16:9', '4:3'] : ['9:16', '3:4'];
    const row2 = wide ? ['2:1', '1:1'] : ['1:2', '1:1'];
    [['phone', 'm', 'text-[11px]'], ['pc', 'p', 'text-[10px]']].forEach(([side, pfx, sz]) => {
      const autoSlot = $(`${pfx}-camAutoSlot`);
      const r1 = $(`${pfx}-camRatioRow1`);
      const r2 = $(`${pfx}-camRatioRow2`);
      const btns1 = r1.querySelectorAll('button');
      if (!autoSlot.firstElementChild || btns1.length !== 2 || btns1[0].dataset.ratio !== row1[0]) {
        autoSlot.innerHTML = buildCamBtns(['Auto'], 'data-ratio', `js-${pfx}-ratio`, `${sz} w-full h-full`);
        r1.innerHTML = buildCamBtns(row1, 'data-ratio', `js-${pfx}-ratio`, `${sz} h-full`);
        r2.innerHTML = buildCamBtns(row2, 'data-ratio', `js-${pfx}-ratio`, `${sz} h-full`);
        bindRatioBtns(side, pfx);
      }
    });
  };

  $$('.js-audio-slot').forEach(el => {
    const lg = el.dataset.size === 'lg';
    const txt = lg ? 'text-[14px]' : 'text-[12px]';
    const val = lg ? 'text-[13px]' : 'text-[11px]';
    const gap = lg ? 'gap-2' : 'gap-1';
    el.innerHTML = `
      <div class="flex items-center justify-between ${txt}"><span>Voice isolation</span><div class="owl-switch is-on js-iso-sw"></div></div>
      <div class="norm-row js-norm-row flex items-center justify-between ${txt}"><span>Auto level</span><div class="owl-switch is-on js-norm-sw"></div></div>
      <div class="flex flex-col ${gap} ${lg ? 'pt-0.5' : ''}"><div class="flex justify-between ${txt}"><span>Noise reduction</span><span class="js-nr-val tabular-nums ${val} text-white/60">5</span></div><div class="fader js-nr-fader"><div class="fader-track"><div class="fader-fill"></div></div><div class="fader-cap"></div><input type="range" class="js-nr-sl" min="0" max="10" value="5" aria-label="Noise reduction" /></div></div>
      <div class="flex flex-col ${gap}"><div class="flex justify-between ${txt}"><span>Volume</span><span class="js-vol-val tabular-nums ${val} text-white/60">0 dB</span></div><div class="fader js-vol-fader"><div class="fader-track"><div class="fader-live"></div><div class="fader-fill"></div></div><div class="fader-cap"></div><input type="range" class="js-vol-sl" min="-12" max="12" step="3" value="0" aria-label="Volume" /></div></div>`;
  });

  const buildTransportFooter = (side) => {
    const m = side === 'phone';
    const h = m ? 'my-2.5 h-[44px] rounded-[8px]' : 'my-2 h-[36px] rounded-[7px]';
    const ic = m ? 'w-[24px] h-[24px]' : 'w-[15px] h-[15px]';
    const hov = m ? '' : 'hover:border-white/35';
    return `<button id="${m ? 'm' : 'p'}-btnRefresh" aria-label="Refresh" class="${h} bg-black border border-white/20 ${hov} text-white/80 flex items-center justify-center cursor-pointer">${iconImg('refresh', `${ic} owl-icon`, 'Refresh')}</button><div class="transport-divider w-0 h-full border-l border-white/15"></div>` +
      CFG.links.map(k => `<button data-app-side="${side}" data-transport="${k}" aria-label="${k}" class="conn-toggle is-on ${h} border flex items-center justify-center cursor-pointer">${iconImg(CFG.icons[k], `${ic} owl-icon`, k)}</button>`).join('');
  };
  $('m-transportFooter').innerHTML = buildTransportFooter('phone');
  $('p-transportFooter').innerHTML = buildTransportFooter('pc');

  $('m-helpGrid').innerHTML = CFG.help.map(h => h.href
    ? `<a href="${h.href}" target="_blank" rel="noopener noreferrer" class="w-full aspect-square bg-black border border-white/15 rounded-[12px] flex flex-col items-center justify-center gap-2 text-white active:bg-white/10">${iconImg(h.ic, 'w-[26px] h-[26px] owl-icon', h.t)}<span class="text-[12px] text-white/70 font-medium">${h.t}</span></a>`
    : `<button data-m-help="${h.k}" class="w-full aspect-square bg-black border border-white/15 rounded-[12px] flex flex-col items-center justify-center gap-2 text-white active:bg-white/10 cursor-pointer">${iconImg(h.ic, 'w-[26px] h-[26px] owl-icon', h.t)}<span class="text-[12px] text-white/70 font-medium">${h.t}</span></button>`
  ).join('');

  $('p-helpGrid').innerHTML = CFG.help.map(h => h.href
    ? `<a href="${h.href}" target="_blank" rel="noopener noreferrer" class="w-full aspect-square bg-black border border-white/15 hover:border-white/30 rounded-[10px] flex flex-col items-center justify-center gap-1.5 text-white transition-colors">${iconImg(h.ic, 'w-[22px] h-[22px] owl-icon', h.t)}<span class="text-[11px] text-white/70 font-medium">${h.t}</span></a>`
    : `<button data-p-help="${h.k}" class="w-full aspect-square bg-black border border-white/15 hover:border-white/30 rounded-[10px] flex flex-col items-center justify-center gap-1.5 text-white transition-colors cursor-pointer">${iconImg(h.ic, 'w-[22px] h-[22px] owl-icon', h.t)}<span class="text-[11px] text-white/70 font-medium">${h.t}</span></button>`
  ).join('');

  $('m-creditsList').innerHTML = CFG.credits.map(([n, u]) =>
    `<a href="https://github.com/${u}" target="_blank" rel="noopener noreferrer" class="flex justify-between px-4 py-3.5 border-b border-white/10 active:bg-white/5"><span>${n}</span><span class="text-[12px] text-white/50">@${u}</span></a>`
  ).join('') + `<a href="https://github.com/diveshpatil9104/owlmic" target="_blank" rel="noopener noreferrer" class="flex justify-between items-center px-4 py-3.5 active:bg-white/5"><span>GitHub</span>${iconImg('external', 'w-3.5 h-3.5 owl-img opacity-55', '↗')}</a>`;

  $('p-creditsList').innerHTML = CFG.credits.map(([n, u]) =>
    `<a href="https://github.com/${u}" target="_blank" rel="noopener noreferrer" class="flex justify-between px-3.5 py-2.5 border-b border-white/10 hover:bg-white/5"><span>${n}</span><span class="text-[11px] text-white/50">@${u}</span></a>`
  ).join('') + `<a href="https://github.com/diveshpatil9104/owlmic" target="_blank" rel="noopener noreferrer" class="flex justify-between items-center px-3.5 py-2.5 hover:bg-white/5"><span>GitHub</span>${iconImg('external', 'w-3 h-3 owl-img opacity-55', '↗')}</a>`;

  bindAllFallbacks();
  lucide.createIcons();

  // ===================== CONNECTION & ONE-WAY PAIRING LOGIC =====================
  const isTrusted = () => state.phoneTrusted && state.pcTrusted;

  const commonLink = () => {
    if (!state.phone.powered) return null;
    return CFG.links.find(k =>
      state.sim.phone[k] && state.sim.pc[k] && state.phone.links[k] && state.pc.links[k]
    ) || null;
  };

  const activeLink = () => {
    if (state.booting || state.phoneBlocked || state.pcBlocked || state.userDisconnected || !isTrusted()) return null;
    return commonLink();
  };

  const formatLastConnected = ts => {
    if (!ts) return null;
    const sec = Math.max(0, Math.floor((Date.now() - ts) / 1000));
    if (sec < 60) return 'Just now';
    if (sec < 3600) return `${Math.floor(sec / 60)}m ago`;
    const d = new Date(ts);
    return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`;
  };

  let bootTimer = null, rotTimer = null, copyTimer = null, flashTimer = null;
  const hudTimers = { phone: null, pc: null };
  let lastLink = null, wasConnected = false;
  let lastMIc = '', lastPIc = '', lastMRowSig = '', lastPRowSig = '';
  let lastMPreviewHidden = null, lastPPreviewHidden = null;

  const triggerSyncFlash = () => {
    clearTimeout(flashTimer);
    $('m-primaryRow').classList.add('is-flash');
    $('p-primaryRow').classList.add('is-flash');
    flashTimer = setTimeout(() => {
      $('m-primaryRow').classList.remove('is-flash');
      $('p-primaryRow').classList.remove('is-flash');
    }, 540);
  };

  // Mobile initiates connection; PC approves only on first-time pairing
  const initiateFromMobile = () => {
    if (state.booting || !commonLink() || state.phoneBlocked || state.pcBlocked) return;
    state.userDisconnected = false;
    if (isTrusted()) {
      state.pairingRequest = false;
      triggerSyncFlash();
      render();
      return;
    }
    if (!state.pairingRequest) {
      state.pairingRequest = true;
      state.pc.flyoutOpen = true;
      state.pc.drawer = 'wifi';
    }
    render();
  };

  const pairOnPc = () => {
    if (!state.pairingRequest || !commonLink() || state.phoneBlocked || state.pcBlocked) return;
    state.pairingRequest = false;
    state.phoneTrusted = true;
    state.pcTrusted = true;
    state.userDisconnected = false;
    triggerSyncFlash();
    render();
  };

  const cancelOrDeclineRequest = () => {
    state.pairingRequest = false;
    state.userDisconnected = true;
    render();
  };

  const disconnectSession = () => {
    if (activeLink()) state.lastConnectedAt = Date.now();
    state.userDisconnected = true;
    state.pairingRequest = false;
    render();
  };

  const forgetOnPc = () => {
    if (activeLink()) state.lastConnectedAt = Date.now();
    state.pcTrusted = false;
    state.pairingRequest = false;
    state.userDisconnected = true;
    render();
  };

  const clearHudTimer = (side) => { clearTimeout(hudTimers[side]); hudTimers[side] = null; };
  const scheduleHudDim = (side) => {
    clearHudTimer(side);
    const dev = state[side];
    if (!state.camera || !dev.camHud || (state.mic && dev.dockOpen)) return;
    hudTimers[side] = setTimeout(() => {
      if (!(state.mic && dev.dockOpen)) {
        dev.camMenu = false;
        dev.camHud = false;
        render();
      }
    }, CFG.hudIdleMs);
  };

  // ===================== EXACT EDGE-TO-EDGE SENSOR & CAMERA ENGINE =====================
  const sensorCanvas = document.createElement('canvas');
  const sensorCtx = sensorCanvas.getContext('2d');

  const getSensorDims = () => {
    const pgW = $('m-playground').clientWidth || 373;
    const pgH = $('m-playground').clientHeight || 544;
    const land = $('deviceMobile').classList.contains('is-landscape');
    const w = land ? pgH : pgW;
    const h = land ? pgW : pgH;
    return { w, h, ar: w / h };
  };

  const getActiveAr = () => {
    const { ar } = getSensorDims();
    return state.ratio === 'Auto' ? ar : CFG.ratios[state.ratio];
  };

  const applyCrop = (layerEl, boxEl, ambEl, targetAr) => {
    const LW = layerEl.clientWidth, LH = layerEl.clientHeight;
    if (LW <= 0 || LH <= 0) return;
    let w = LW, h = LH, mode = 'none';
    if (targetAr && Math.abs(LW / LH - targetAr) >= 0.01) {
      if (LW / LH > targetAr) { w = Math.round(LH * targetAr); mode = 'pillar'; }
      else { h = Math.round(LW / targetAr); mode = 'letter'; }
    }
    boxEl.style.width = `${w}px`;
    boxEl.style.height = `${h}px`;
    boxEl.dataset.crop = mode;
    ambEl.classList.toggle('is-visible', mode !== 'none');
  };

  const syncAllCrops = () => {
    const activeAr = getActiveAr();
    applyCrop($('m-camLayer'), $('m-camBox'), $('m-camAmbWrap'), state.ratio === 'Auto' ? 0 : activeAr);
    applyCrop($('p-camLayer'), $('p-camBox'), $('p-camAmbWrap'), activeAr);
  };

  let ct = 0, blink = 0, nextBlink = 180;
  const drawPerson = (ctx, W, H, mir) => {
    ctx.save();
    if (mir) { ctx.translate(W, 0); ctx.scale(-1, 1); }
    const S = (Math.min(W, H) / 340) * (state.lens === 'Back' ? 0.94 : 1);
    const bg = ctx.createLinearGradient(0, 0, W, H);
    bg.addColorStop(0, '#38bdf8'); bg.addColorStop(0.55, '#6366f1'); bg.addColorStop(1, '#1e1b4b');
    ctx.fillStyle = bg; ctx.fillRect(0, 0, W, H);

    const glow = ctx.createRadialGradient(W*.28, H*.24, 10*S, W*.28, H*.24, Math.max(W, H)*.55);
    glow.addColorStop(0, 'rgba(255,245,214,.35)'); glow.addColorStop(1, 'rgba(255,245,214,0)');
    ctx.fillStyle = glow; ctx.fillRect(0, 0, W, H);

    const cx = W * 0.5 + Math.sin(ct * 0.65) * 3 * S, hy = H * 0.44 + Math.sin(ct * 1.15) * 2.2 * S, R = 56 * S;
    ctx.fillStyle = '#1f2335'; ctx.beginPath(); ctx.ellipse(cx, hy - 8*S, R*1.12, R*1.15, 0, 0, Math.PI*2); ctx.fill();

    const shY = hy + R*1.36, sh = ctx.createLinearGradient(cx - 120*S, shY, cx + 120*S, H);
    sh.addColorStop(0, '#fb7185'); sh.addColorStop(0.6, '#f43f5e'); sh.addColorStop(1, '#9f1239');
    ctx.fillStyle = sh; ctx.beginPath();
    ctx.moveTo(cx - 32*S, shY);
    ctx.bezierCurveTo(cx - 95*S, shY + 14*S, cx - 165*S, shY + 55*S, cx - 190*S, H + 24*S);
    ctx.lineTo(cx + 190*S, H + 24*S);
    ctx.bezierCurveTo(cx + 165*S, shY + 55*S, cx + 95*S, shY + 14*S, cx + 32*S, shY);
    ctx.closePath(); ctx.fill();
    ctx.strokeStyle = 'rgba(255,255,255,.24)'; ctx.lineWidth = 3*S; ctx.beginPath(); ctx.arc(cx, shY + 6*S, 36*S, .15*Math.PI, .85*Math.PI); ctx.stroke();

    const nt = hy + R*0.72;
    ctx.fillStyle = '#e7a982'; ctx.beginPath(); ctx.roundRect(cx - 21*S, nt, 42*S, 48*S, 16*S); ctx.fill();
    ctx.fillStyle = 'rgba(130,62,34,.24)'; ctx.beginPath(); ctx.ellipse(cx, nt + 14*S, 21*S, 12*S, 0, 0, Math.PI*2); ctx.fill();

    ctx.fillStyle = '#edb08a'; ctx.beginPath();
    ctx.ellipse(cx - R*.94, hy + 6*S, 10*S, 15*S, -0.1, 0, Math.PI*2);
    ctx.ellipse(cx + R*.94, hy + 6*S, 10*S, 15*S, 0.1, 0, Math.PI*2); ctx.fill();

    const fc = ctx.createRadialGradient(cx - 18*S, hy - 22*S, 12*S, cx, hy + 6*S, R*1.25);
    fc.addColorStop(0, '#ffd4b5'); fc.addColorStop(0.65, '#f2b68f'); fc.addColorStop(1, '#d89068');
    ctx.fillStyle = fc; ctx.beginPath(); ctx.ellipse(cx, hy + 2*S, R*.92, R*1.06, 0, 0, Math.PI*2); ctx.fill();
    ctx.fillStyle = 'rgba(244,114,140,.22)'; ctx.beginPath();
    ctx.ellipse(cx - 26*S, hy + 17*S, 12*S, 7*S, 0, 0, Math.PI*2);
    ctx.ellipse(cx + 26*S, hy + 17*S, 12*S, 7*S, 0, 0, Math.PI*2); ctx.fill();

    const ey = hy + 2*S, eh = Math.max(1.4*S, 7.5*S*(1 - blink));
    ctx.fillStyle = '#1e1b2e'; ctx.beginPath();
    ctx.ellipse(cx - 20*S, ey, 6*S, eh, 0, 0, Math.PI*2);
    ctx.ellipse(cx + 20*S, ey, 6*S, eh, 0, 0, Math.PI*2); ctx.fill();
    if (blink < 0.5) {
      ctx.fillStyle = '#FFF'; ctx.beginPath();
      ctx.arc(cx - 22*S, ey - 2.2*S, 2.1*S, 0, Math.PI*2);
      ctx.arc(cx + 18*S, ey - 2.2*S, 2.1*S, 0, Math.PI*2); ctx.fill();
    }
    ctx.strokeStyle = '#2c2638'; ctx.lineWidth = 3.2*S; ctx.lineCap = 'round'; ctx.beginPath();
    ctx.moveTo(cx - 29*S, ey - 13*S); ctx.quadraticCurveTo(cx - 20*S, ey - 17*S, cx - 11*S, ey - 13*S);
    ctx.moveTo(cx + 11*S, ey - 13*S); ctx.quadraticCurveTo(cx + 20*S, ey - 17*S, cx + 29*S, ey - 13*S); ctx.stroke();

    ctx.strokeStyle = '#9c523b'; ctx.lineWidth = 2.8*S; ctx.beginPath(); ctx.arc(cx, hy + 22*S, 13*S, .2*Math.PI, .8*Math.PI); ctx.stroke();

    const hr = ctx.createLinearGradient(cx - R, hy - R*1.2, cx + R, hy);
    hr.addColorStop(0, '#313756'); hr.addColorStop(1, '#181b2a');
    ctx.fillStyle = hr; ctx.beginPath();
    ctx.arc(cx, hy - 8*S, R*.99, Math.PI*1.05, Math.PI*1.98);
    ctx.quadraticCurveTo(cx + 26*S, hy - 24*S, cx - 10*S, hy - 28*S);
    ctx.quadraticCurveTo(cx - 42*S, hy - 30*S, cx - R*.95, hy - 4*S); ctx.closePath(); ctx.fill();
    ctx.restore();
  };

  const blitFrame = (boxEl, mainCanvas, ambCanvas, sx, sy, sw, sh, fullAmb) => {
    const FW = boxEl.clientWidth, FH = boxEl.clientHeight;
    if (FW <= 0 || FH <= 0) return;
    const dpr = Math.min(devicePixelRatio || 1, 2);
    const cw = Math.round(FW * dpr), ch = Math.round(FH * dpr);
    if (mainCanvas.width !== cw || mainCanvas.height !== ch) { mainCanvas.width = cw; mainCanvas.height = ch; }
    mainCanvas.getContext('2d').drawImage(sensorCanvas, sx, sy, sw, sh, 0, 0, cw, ch);

    if (boxEl.dataset.crop !== 'none') {
      const aw = 140, ah = 140;
      if (ambCanvas.width !== aw || ambCanvas.height !== ah) { ambCanvas.width = aw; ambCanvas.height = ah; }
      const aCtx = ambCanvas.getContext('2d');
      if (fullAmb) aCtx.drawImage(sensorCanvas, 0, 0, sensorCanvas.width, sensorCanvas.height, 0, 0, aw, ah);
      else aCtx.drawImage(sensorCanvas, sx, sy, sw, sh, 0, 0, aw, ah);
    }
  };

  const renderCamerasNow = () => {
    if (!state.camera || !activeLink()) return;
    const drawM = state.phone.powered && !state.phone.camHidden;
    const drawP = state.pc.flyoutOpen && !state.pc.camHidden;
    if (!drawM && !drawP) return;
    const { w, h } = getSensorDims();
    const scale = Math.min(devicePixelRatio || 1, 2);
    const SW = Math.round(w * scale), SH = Math.round(h * scale);
    if (sensorCanvas.width !== SW || sensorCanvas.height !== SH) { sensorCanvas.width = SW; sensorCanvas.height = SH; }

    const mir = state.mirror && state.lens === 'Front';
    drawPerson(sensorCtx, SW, SH, mir);

    const activeAr = getActiveAr();
    let sx = 0, sy = 0, sw = SW, sh = SH;
    if (Math.abs(SW / SH - activeAr) >= 0.01) {
      if (SW / SH > activeAr) { sw = Math.round(SH * activeAr); sx = Math.round((SW - sw) / 2); }
      else { sh = Math.round(SW / activeAr); sy = Math.round((SH - sh) / 2); }
    }

    if (drawM) blitFrame($('m-camBox'), $('m-camCanvas'), $('m-camAmbCanvas'), sx, sy, sw, sh, true);
    if (drawP) blitFrame($('p-camBox'), $('p-camCanvas'), $('p-camAmbCanvas'), sx, sy, sw, sh, false);
  };

  const syncMobilePg = () => {
    const w = $('m-playground').clientWidth, h = $('m-playground').clientHeight;
    if (w <= 0 || h <= 0) return;
    const root = document.documentElement.style;
    root.setProperty('--pg-w', `${w}px`);
    root.setProperty('--pg-h', `${h}px`);
    syncAllCrops();
    renderCamerasNow();
  };

  const fitPhoneStage = () => {
    const z = $('phoneZone');
    const availW = Math.max(200, z.clientWidth - 48);
    const availH = Math.max(200, z.clientHeight - 36);
    const needW = state.phone.isLandscape ? 828 : 391;
    const needH = state.phone.isLandscape ? 391 : 828;
    const scale = Math.min(0.96, availW / needW, availH / needH);
    $('phoneScaler').style.transform = `scale(${scale.toFixed(3)})`;
  };

  new ResizeObserver(syncMobilePg).observe($('m-playground'));
  new ResizeObserver(() => { syncAllCrops(); renderCamerasNow(); }).observe($('p-playground'));
  new ResizeObserver(fitPhoneStage).observe($('phoneZone'));

  // ===================== AUDIO WAVEFORM & MAIN LOOP =====================
  const mWaveBars = Array.from({ length: CFG.bars }, () => { const b = document.createElement('div'); b.className = 'wave-bar'; $('m-waveBars').appendChild(b); return b; });
  const pWaveBars = Array.from({ length: CFG.bars }, () => { const b = document.createElement('div'); b.className = 'wave-bar'; $('p-waveBars').appendChild(b); return b; });
  let wt = 0, wTgt = 0, normLvl = 0, wFrm = 0;

  (function mainLoop() {
    ct += 0.028; wt += 0.055; wFrm++;
    const ok = Boolean(activeLink());

    if (ok && state.camera) {
      if (--nextBlink <= 0) { blink = Math.min(1, blink + 0.24); if (blink >= 1) nextBlink = 140 + Math.random() * 140; }
      else if (blink > 0) blink = Math.max(0, blink - 0.22);
      renderCamerasNow();
    }

    if (ok && state.mic) {
      if (Math.random() < 0.14) wTgt = Math.random() * 0.82 + 0.12;
      else if (Math.random() < 0.2) wTgt = 0.06;
      state.audioLevel += (wTgt - state.audioLevel) * 0.24;
      const v = Math.min(1, Math.max(0, (state.audioLevel - 0.08) / 0.92));
      const loud = Math.min(1, Math.pow(v, 0.7) * 1.2);
      normLvl += (loud - normLvl) * 0.18;

      const mid = (CFG.bars - 1) / 2;
      const mDocked = $('m-waveDock').classList.contains('is-docked');
      const pDocked = $('p-waveDock').classList.contains('is-docked');
      const mMin = mDocked ? 4 : 5, mMax = mDocked ? 24 : 76;
      const pMin = pDocked ? 3 : 4, pMax = pDocked ? 18 : 60;

      for (let i = 0; i < CFG.bars; i++) {
        const env = Math.max(0.2, Math.cos((Math.abs(i - mid) / mid) * Math.PI * 0.46));
        const mot = (Math.sin(wt * 5.2 + i * 0.52) + Math.sin(wt * 9.1 + i * 1.1) * 0.5) / 1.5;
        const normH = Math.min(1, Math.max(0.08, (loud * 0.95 + (mot + 1) * 0.14 * (0.2 + loud)) * env));
        mWaveBars[i].style.height = `${(mMin + (mMax - mMin) * normH).toFixed(1)}px`;
        if (state.pc.flyoutOpen) pWaveBars[i].style.height = `${(pMin + (pMax - pMin) * normH).toFixed(1)}px`;
      }
      if (wFrm % 4 === 0) $$('.js-vol-fader').forEach(f => f.style.setProperty('--live', `${Math.round(Math.min(100, normLvl * 100))}%`));
    } else {
      state.audioLevel = 0; normLvl = 0;
    }
    requestAnimationFrame(mainLoop);
  })();

  // ===================== UI HELPERS =====================
  const mHelpViews = { bento: $('m-helpBento'), howto: $('m-helpHowto'), credits: $('m-helpCredits'), privacy: $('m-helpPrivacy') };
  const pHelpViews = { bento: $('p-helpBento'), howto: $('p-helpHowto'), credits: $('p-helpCredits'), privacy: $('p-helpPrivacy') };
  const showHelp = (map, k) => Object.entries(map).forEach(([key, el]) => {
    el.classList.toggle('hidden', key !== k); el.classList.toggle('flex', key === k);
  });

  const toggleAddForm = open => {
    $('m-addHostForm').classList.toggle('hidden', !open);
    $('m-addHostForm').classList.toggle('flex', open);
    if ($('m-btnShowAdd')) $('m-btnShowAdd').classList.toggle('hidden', open);
    if (open) $('m-addPcInput').focus(); else $('m-addPcInput').value = '';
  };

  const flipRatioForOrient = land => {
    if (state.ratio === '16:9' || state.ratio === '9:16') state.ratio = land ? '16:9' : '9:16';
    else if (state.ratio === '4:3' || state.ratio === '3:4') state.ratio = land ? '4:3' : '3:4';
    else if (state.ratio === '2:1' || state.ratio === '1:2') state.ratio = land ? '2:1' : '1:2';
  };

  const setOrientation = land => {
    clearTimeout(rotTimer);
    state.phone.isLandscape = land;
    if (land) $('m-notifShade').classList.remove('is-open');
    $('deviceMobile').classList.toggle('is-landscape-chassis', land);
    $('deviceMobile').classList.toggle('is-portrait-chassis', !land);
    fitPhoneStage();
    render();
    rotTimer = setTimeout(() => {
      flipRatioForOrient(land);
      $('deviceMobile').classList.toggle('is-landscape', land);
      $('deviceMobile').classList.toggle('is-portrait', !land);
      render();
      requestAnimationFrame(() => { syncMobilePg(); syncAllCrops(); renderCamerasNow(); });
    }, 230);
  };

  const runBoot = () => {
    clearTimeout(bootTimer);
    state.booting = true;
    render();
    bootTimer = setTimeout(() => {
      state.booting = false;
      const avail = commonLink();
      if (avail && !state.phoneBlocked && !state.pcBlocked && !state.userDisconnected) {
        if (isTrusted()) triggerSyncFlash();
        else {
          state.pairingRequest = true;
          state.pc.flyoutOpen = true;
          state.pc.drawer = 'wifi';
        }
      }
      render();
    }, 650);
  };

  const syncAudio = () => {
    const nrP = `${state.nr * 10}%`;
    const pos = ((state.vol + 12) / 24) * 100;
    const fillLeft = `${Math.min(50, pos)}%`;
    const fillWidth = `${Math.abs(pos - 50)}%`;
    const volText = state.vol > 0 ? `+${state.vol} dB` : `${state.vol} dB`;
    $$('.js-nr-fader').forEach(f => { f.style.setProperty('--left', '0%'); f.style.setProperty('--width', nrP); f.style.setProperty('--pos', nrP); });
    $$('.js-nr-sl').forEach(s => s.value = state.nr);
    $$('.js-nr-val').forEach(el => el.textContent = state.nr);
    $$('.js-vol-fader').forEach(f => { f.style.setProperty('--left', fillLeft); f.style.setProperty('--width', fillWidth); f.style.setProperty('--pos', `${pos}%`); });
    $$('.js-vol-sl').forEach(s => s.value = state.vol);
    $$('.js-vol-val').forEach(el => el.textContent = volText);
    $$('.js-iso-sw').forEach(el => el.classList.toggle('is-on', state.voiceIso));
    $$('.js-norm-sw').forEach(el => el.classList.toggle('is-on', state.normalization));
  };

  // Clean, borderless connection rows (no pills, no duplicate transport text)
  const renderConnRow = (side, avail, link) => {
    const isM = side === 'phone';
    const row = $(isM ? 'm-primaryRow' : 'p-primaryRow');
    const nameEl = $(isM ? 'm-pcName' : 'p-phoneName');
    const metaEl = $(isM ? 'm-pcMeta' : 'p-phoneMeta');
    const icWrap = $(isM ? 'm-pcLinkIc' : 'p-phoneLinkIc');
    const actEl = $(isM ? 'm-pcActions' : 'p-phoneActions');
    const ip = isM ? '192.168.1.15:7653' : '192.168.1.42:7653';
    const localBlocked = isM ? state.phoneBlocked : state.pcBlocked;
    const remoteBlocked = isM ? state.pcBlocked : state.phoneBlocked;

    const ok = Boolean(link);
    const pending = Boolean(state.pairingRequest && avail && !localBlocked && !remoteBlocked);
    const lastStr = formatLastConnected(state.lastConnectedAt);

    let mode = 'offline';
    if (localBlocked) mode = 'localBlocked';
    else if (remoteBlocked) mode = 'remoteBlocked';
    else if (pending) mode = 'pending';
    else if (ok) mode = 'connected';
    else if (avail) mode = 'ready';

    const icKey = ok ? link : (pending ? avail : 'Disconnected');
    const canTapRow = isM && mode === 'ready';

    row.classList.toggle('is-pairing', pending || state.booting);
    row.classList.toggle('cursor-pointer', canTapRow);
    row.dataset.rowConnect = canTapRow ? '1' : '';

    nameEl.className = `${isM ? 'text-[15px]' : 'text-[14px]'} font-medium leading-snug truncate ${ok || pending || (isM && mode === 'ready') ? 'text-white' : 'text-white/50'}`;
    icWrap.className = `conn-row-ic ${isM ? 'w-5 h-5' : 'w-[18px] h-[18px]'} flex items-center justify-center shrink-0 transition-opacity ${ok ? 'opacity-100' : pending ? 'opacity-85' : 'opacity-25'}`;

    const metaCls = `tabular-nums ${isM ? 'text-[13px]' : 'text-[12px]'} leading-snug mt-0.5 truncate`;
    if (mode === 'localBlocked') {
      metaEl.textContent = 'Blocked';
      metaEl.className = `${metaCls} text-white/35`;
    } else if (mode === 'remoteBlocked') {
      metaEl.textContent = `Blocked by ${isM ? 'PC' : 'phone'}`;
      metaEl.className = `${metaCls} text-white/35`;
    } else if (mode === 'pending') {
      metaEl.textContent = isM ? 'Waiting for PC' : 'Wants to pair';
      metaEl.className = `${metaCls} text-white/70`;
    } else if (mode === 'connected') {
      metaEl.textContent = ip;
      metaEl.className = `${metaCls} text-white/50`;
    } else if (lastStr) {
      metaEl.textContent = lastStr;
      metaEl.className = `${metaCls} text-white/35`;
    } else if (mode === 'ready') {
      metaEl.textContent = ip;
      metaEl.className = `${metaCls} text-white/45`;
    } else {
      metaEl.textContent = isM ? 'Offline' : (state.pcTrusted ? 'Offline' : ip);
      metaEl.className = `${metaCls} text-white/35`;
    }

    const sig = `${mode}|${icKey}|${state.pcTrusted ? 1 : 0}`;
    if (isM) {
      if (sig === lastMRowSig) return;
      lastMRowSig = sig;
    } else {
      if (sig === lastPRowSig) return;
      lastPRowSig = sig;
    }

    const icSz = isM ? 'w-[19px] h-[19px]' : 'w-[17px] h-[17px]';
    icWrap.innerHTML = iconImg(CFG.icons[icKey] || 'globe', `${icSz} owl-img`, icKey);
    bindAllFallbacks(icWrap);

    if (isM) {
      if (mode === 'localBlocked') {
        actEl.innerHTML = `<button data-conn-act="unblock" data-side="phone" class="py-1 text-[13.5px] font-medium text-white active:opacity-70 cursor-pointer">Unblock</button>`;
      } else if (mode === 'remoteBlocked') {
        actEl.innerHTML = '';
      } else if (mode === 'pending') {
        actEl.innerHTML = `<button data-conn-act="cancel" data-side="phone" aria-label="Cancel" class="w-8 h-8 rounded-[8px] bg-white/10 text-white/70 active:text-white flex items-center justify-center cursor-pointer">${iconImg('close', 'w-4 h-4 owl-icon', 'Cancel')}</button>`;
      } else if (mode === 'connected') {
        actEl.innerHTML =
          `<button data-conn-act="disconnect" data-side="phone" aria-label="Disconnect" class="w-8 h-8 flex items-center justify-center opacity-70 active:opacity-100 cursor-pointer">${iconImg('unplug', 'w-[17px] h-[17px] owl-icon', 'Disconnect')}</button>` +
          `<button data-conn-act="block" data-side="phone" class="py-1 text-[13px] text-white/50 active:text-white cursor-pointer">Block</button>`;
      } else if (mode === 'ready') {
        actEl.innerHTML = `<button data-conn-act="connect" data-side="phone" aria-label="Connect" class="w-8 h-8 rounded-[8px] bg-white text-black flex items-center justify-center active:bg-white/80 cursor-pointer transition-all">${iconImg('link', 'w-4 h-4 owl-icon', 'Connect')}</button>`;
      } else {
        actEl.innerHTML = `<button data-conn-act="block" data-side="phone" class="py-1 text-[13px] text-white/45 active:text-white cursor-pointer">Block</button>`;
      }
    } else {
      if (mode === 'localBlocked') {
        actEl.innerHTML = `<button data-conn-act="unblock" data-side="pc" class="py-1 text-[13px] font-medium text-white hover:opacity-80 cursor-pointer transition-opacity">Unblock</button>`;
      } else if (mode === 'remoteBlocked') {
        actEl.innerHTML = '';
      } else if (mode === 'pending') {
        actEl.innerHTML =
          `<button data-conn-act="pair" data-side="pc" aria-label="Pair" class="w-7 h-7 rounded-[6px] bg-white text-black flex items-center justify-center hover:bg-white/90 cursor-pointer transition-all">${iconImg('check', 'w-3.5 h-3.5 owl-icon', 'Pair')}</button>` +
          `<button data-conn-act="decline" data-side="pc" aria-label="Decline" class="w-7 h-7 rounded-[6px] bg-white/10 text-white/50 hover:text-white flex items-center justify-center cursor-pointer transition-all">${iconImg('close', 'w-3.5 h-3.5 owl-icon', 'Decline')}</button>`;
      } else if (mode === 'connected') {
        actEl.innerHTML =
          `<button data-conn-act="disconnect" data-side="pc" aria-label="Disconnect" class="w-7 h-7 flex items-center justify-center opacity-65 hover:opacity-100 cursor-pointer transition-opacity">${iconImg('unplug', 'w-[15px] h-[15px] owl-icon', 'Disconnect')}</button>` +
          `<button data-conn-act="block" data-side="pc" class="py-1 text-[12.5px] text-white/45 hover:text-white cursor-pointer transition-colors">Block</button>`;
      } else if (state.pcTrusted) {
        actEl.innerHTML = `<button data-conn-act="forget" data-side="pc" class="py-1 text-[12.5px] text-white/50 hover:text-white cursor-pointer transition-colors">Forget</button>`;
      } else {
        actEl.innerHTML = `<button data-conn-act="block" data-side="pc" class="py-1 text-[12.5px] text-white/45 hover:text-white cursor-pointer transition-colors">Block</button>`;
      }
    }
    bindAllFallbacks(actEl);
  };

  const renderWaveDock = (dockEl, quickEl, dev) => {
    if (state.mic) {
      const wasAct = dockEl.classList.contains('is-active');
      const wasD = dockEl.classList.contains('is-docked');
      const d = state.camera;
      if (!d) dev.dockOpen = false; else dev.micQuick = false;
      const quickOpen = !d && !dev.drawer && dev.micQuick;
      dockEl.classList.toggle('is-animating', (wasAct && wasD !== d) || d);
      dockEl.classList.add('is-active');
      dockEl.classList.toggle('is-docked', d);
      dockEl.classList.toggle('is-centered', !d);
      dockEl.classList.toggle('is-lifted', quickOpen);
      dockEl.classList.toggle('is-expanded', d && dev.dockOpen);
      quickEl.classList.toggle('is-open', quickOpen);
    } else {
      dev.dockOpen = dev.micQuick = false;
      dockEl.classList.remove('is-animating', 'is-active', 'is-centered', 'is-lifted', 'is-docked', 'is-expanded');
      quickEl.classList.remove('is-open');
    }
  };

  const setOptionActive = (nodes, matchVal, key) => nodes.forEach(b => {
    const a = b.dataset[key] === matchVal;
    b.classList.toggle('bg-white', a);
    b.classList.toggle('text-black', a);
    b.classList.toggle('font-medium', true);
    b.classList.toggle('text-white/65', !a);
    b.classList.toggle('hover:bg-white/5', !a);
    b.classList.toggle('hover:text-white', !a);
  });

  // ===================== MASTER RENDER =====================
  function render() {
    const avail = commonLink();
    if (!avail || state.phoneBlocked || state.pcBlocked) state.pairingRequest = false;

    // Auto-initiate first-time request if link comes up while not manually disconnected
    if (!state.booting && avail && !state.phoneBlocked && !state.pcBlocked && !state.userDisconnected && !isTrusted() && !state.pairingRequest) {
      state.pairingRequest = true;
      state.pc.flyoutOpen = true;
      state.pc.drawer = 'wifi';
    }

    const link = activeLink();
    const ok = Boolean(link);

    if (wasConnected && !ok) state.lastConnectedAt = Date.now();
    if (ok && lastLink && lastLink !== link) triggerSyncFlash();
    wasConnected = ok;
    lastLink = link;

    if (!ok) {
      clearHudTimer('phone'); clearHudTimer('pc');
      Object.assign(state, { camera: false, mic: false, speaker: false });
      Object.assign(state.phone, { camHud: false, camMenu: false, micQuick: false, dockOpen: false });
      Object.assign(state.pc, { camHud: false, camMenu: false, micQuick: false, dockOpen: false });
    }

    const qDisp = state.quality === '480p' ? '480p' : state.quality === 'HD' ? '720p' : '1080p';
    const pending = Boolean(state.pairingRequest && avail && !state.phoneBlocked && !state.pcBlocked);

    // Mobile State
    $('deviceMobile').classList.toggle('is-powered-off', !state.phone.powered);
    $('m-appLayout').classList.toggle('is-disconnected', !ok);
    $('m-appLayout').classList.toggle('is-pulsing', !ok);
    $('m-sbWifi').style.opacity = state.sim.phone['Wi-Fi'] ? '1' : '0.22';

    const mHdrK = link || 'Disconnected';
    if (mHdrK !== lastMIc) {
      lastMIc = mHdrK;
      $('m-headerTransport').innerHTML = iconImg(CFG.icons[mHdrK] || 'globe', 'w-[26px] h-[26px] owl-icon', mHdrK);
      bindAllFallbacks($('m-headerTransport'));
    }

    $('m-btnHelp').classList.toggle('is-on', state.phone.drawer === 'help');
    $('m-btnWifi').classList.toggle('is-on', state.phone.drawer === 'wifi');
    $('m-btnCam').classList.toggle('is-on', state.camera);
    $('m-btnMic').classList.toggle('is-on', state.mic);
    $('m-btnSpk').classList.toggle('is-on', state.speaker);
    $('m-watermark').classList.toggle('opacity-0', (state.camera && !state.phone.camHidden) || state.mic);

    $('m-camLayer').classList.toggle('opacity-100', state.camera);
    $('m-camLayer').classList.toggle('pointer-events-auto', state.camera);
    $('m-camLayer').classList.toggle('opacity-0', !state.camera);
    $('m-camLayer').classList.toggle('pointer-events-none', !state.camera);
    $('m-camLayer').classList.toggle('is-preview-hidden', state.phone.camHidden);

    const mHudOn = state.phone.camHud || state.phone.camMenu || (state.camera && state.mic && state.phone.dockOpen);
    syncRatioMenus();
    const wideRot = ratioIsWide() ? 'rotate(90deg)' : 'rotate(0deg)';
    $('m-camHud').classList.toggle('is-quiet', !mHudOn);
    $('m-camMenuBox').classList.toggle('is-open', state.phone.camMenu);
    $('m-camRatioLabel').textContent = state.ratio;
    $('m-btnPreview').classList.toggle('is-on', state.phone.camHidden);
    $('m-btnPreview').setAttribute('aria-label', state.phone.camHidden ? 'Show preview' : 'Hide preview');
    if (state.phone.camHidden !== lastMPreviewHidden) {
      lastMPreviewHidden = state.phone.camHidden;
      $('m-btnPreview').innerHTML = iconImg(state.phone.camHidden ? 'eye-off' : 'eye', 'w-[15px] h-[15px] owl-icon', 'Preview');
      bindAllFallbacks($('m-btnPreview'));
    }
    $('m-btnMirror').classList.toggle('is-on', state.mirror);
    $('m-camOrientIc').style.transform = wideRot;
    setOptionActive($$('.js-m-ratio'), state.ratio, 'ratio');

    renderWaveDock($('m-waveDock'), $('m-micQuick'), state.phone);
    renderConnRow('phone', avail, link);

    $('m-drawer').classList.toggle('is-open', Boolean(state.phone.drawer));
    $('m-drawerWifi').classList.toggle('hidden', state.phone.drawer !== 'wifi');
    $('m-drawerWifi').classList.toggle('flex', state.phone.drawer === 'wifi');
    $('m-drawerHelp').classList.toggle('hidden', state.phone.drawer !== 'help');
    $('m-drawerHelp').classList.toggle('flex', state.phone.drawer === 'help');

    $('m-notifDesc').textContent = !ok
      ? (pending ? 'Waiting for PC' : 'Disconnected')
      : state.camera && state.mic ? `Mic & ${qDisp} camera · ${link}`
      : state.mic ? `Mic active · ${link}`
      : state.camera ? `${qDisp} camera · ${link}`
      : `Connected · ${link}`;

    // PC State
    $('pcFlyoutAnchor').classList.toggle('is-hidden', !state.pc.flyoutOpen);
    $('pcFlyoutAnchor').classList.toggle('is-tray-open', state.pc.trayOverflowOpen);
    $('btnTrayOwl').classList.toggle('is-open', state.pc.flyoutOpen);
    $('trayOverflowFlyout').classList.toggle('is-hidden', !state.pc.trayOverflowOpen);
    $('btnTrayChevron').classList.toggle('is-open', state.pc.trayOverflowOpen);
    $('p-bento').classList.toggle('is-disconnected', !ok);
    $('p-bento').classList.toggle('is-pulsing', !ok || pending);

    const pTrayK = link || 'Disconnected';
    if (pTrayK !== lastPIc) {
      lastPIc = pTrayK;
      $('p-trayTransport').innerHTML = iconImg(CFG.icons[pTrayK] || 'globe', 'w-[21px] h-[21px] owl-icon', pTrayK);
      bindAllFallbacks($('p-trayTransport'));
    }

    $('p-btnWifi').classList.toggle('is-on', state.pc.drawer === 'wifi');
    $('p-btnCam').classList.toggle('is-on', state.camera);
    $('p-btnMic').classList.toggle('is-on', state.mic);
    $('p-btnSpk').classList.toggle('is-on', state.speaker);
    $('p-btnHelp').classList.toggle('is-on', state.pc.drawer === 'help');
    $('p-watermark').classList.toggle('opacity-0', (state.camera && !state.pc.camHidden) || state.mic);

    $('p-camLayer').classList.toggle('opacity-100', state.camera);
    $('p-camLayer').classList.toggle('pointer-events-auto', state.camera);
    $('p-camLayer').classList.toggle('opacity-0', !state.camera);
    $('p-camLayer').classList.toggle('pointer-events-none', !state.camera);
    $('p-camLayer').classList.toggle('is-preview-hidden', state.pc.camHidden);

    const pHudOn = state.pc.camHud || state.pc.camMenu || (state.camera && state.mic && state.pc.dockOpen);
    $('p-camHud').classList.toggle('is-quiet', !pHudOn);
    $('p-camMenuBox').classList.toggle('is-open', state.pc.camMenu);
    $('p-camRatioLabel').textContent = state.ratio;
    $('p-btnPreview').classList.toggle('is-on', state.pc.camHidden);
    $('p-btnPreview').setAttribute('aria-label', state.pc.camHidden ? 'Show preview' : 'Hide preview');
    if (state.pc.camHidden !== lastPPreviewHidden) {
      lastPPreviewHidden = state.pc.camHidden;
      $('p-btnPreview').innerHTML = iconImg(state.pc.camHidden ? 'eye-off' : 'eye', 'w-[13px] h-[13px] owl-icon', 'Preview');
      bindAllFallbacks($('p-btnPreview'));
    }
    $('p-btnMirror').classList.toggle('is-on', state.mirror);
    $('p-camOrientIc').style.transform = wideRot;
    setOptionActive($$('.js-p-ratio'), state.ratio, 'ratio');

    renderWaveDock($('p-waveDock'), $('p-micQuick'), state.pc);
    renderConnRow('pc', avail, link);

    $('p-drawer').classList.toggle('is-open', Boolean(state.pc.drawer));
    $('p-drawerWifi').classList.toggle('hidden', state.pc.drawer !== 'wifi');
    $('p-drawerWifi').classList.toggle('flex', state.pc.drawer === 'wifi');
    $('p-drawerHelp').classList.toggle('hidden', state.pc.drawer !== 'help');
    $('p-drawerHelp').classList.toggle('flex', state.pc.drawer === 'help');

    const tbNet = $('tbNetIc'), tbVol = $('tbVolIc');
    if (tbNet) tbNet.style.opacity = state.sim.pc['Wi-Fi'] ? '1' : '0.28';
    if (tbVol) tbVol.style.opacity = state.speaker ? '1' : '0.55';

    $$('[data-app-side]').forEach(b => {
      b.classList.toggle('is-on', state[b.dataset.appSide].links[b.dataset.transport]);
    });

    // Merged 2-Column Sandbox Panel
    $('s-phonePower').classList.toggle('is-on', state.phone.powered);
    $('s-phoneRotate').classList.toggle('is-on', state.phone.isLandscape);
    $('s-phoneStat').textContent = !state.phone.powered ? 'Off' : (state.phone.isLandscape ? 'Landscape' : 'Portrait');
    $('s-pcFlyout').classList.toggle('is-on', state.pc.flyoutOpen);
    $('s-pcStat').textContent = state.pc.flyoutOpen ? 'Open' : 'Hidden';

    $$('[data-sim-side]').forEach(b => {
      const side = b.dataset.simSide, k = b.dataset.simLink;
      const on = state.sim[side][k];
      b.classList.toggle('is-on', on);
      b.classList.toggle('is-active', Boolean(ok && link === k));
    });

    $('s-hud').classList.toggle('is-on', state.camera && (mHudOn || pHudOn));

    $('s-syncDot').className = `w-2 h-2 rounded-full shrink-0 transition-colors ${ok ? 'bg-white' : pending ? 'bg-white/75 animate-pulse' : 'bg-white/25'}`;
    $('s-syncLabel').textContent = state.booting ? 'Syncing'
      : ok ? `Synced · ${link}`
      : state.phoneBlocked || state.pcBlocked ? 'Blocked'
      : pending ? 'Pair on PC'
      : !state.phone.powered ? 'Phone Off'
      : avail ? `Ready · ${avail}`
      : 'No Link';

    syncAllCrops();
    if (ok && state.camera) renderCamerasNow();
  }

  // ===================== EVENT BINDINGS =====================
  $('s-phonePower').onclick = () => {
    state.phone.powered = !state.phone.powered;
    if (state.phone.powered) runBoot(); else render();
  };
  $('m-screenOff').onclick = () => {
    if (!state.phone.powered) { state.phone.powered = true; runBoot(); }
  };
  $('s-phoneRotate').onclick = () => setOrientation(!state.phone.isLandscape);
  $('s-pcFlyout').onclick = () => { state.pc.flyoutOpen = !state.pc.flyoutOpen; render(); };

  $('s-phoneReset').onclick = () => {
    clearHudTimer('phone');
    const wasLand = state.phone.isLandscape;
    Object.assign(state, {
      phoneTrusted: false, pairingRequest: false, userDisconnected: false,
      phoneBlocked: false, camera: false, mic: false, speaker: false
    });
    state.sim.phone = { USB: false, Tethering: false, 'Wi-Fi': true, Bluetooth: false };
    state.phone = {
      powered: true, isLandscape: false,
      drawer: null, camHud: false, camMenu: false, camHidden: false, micQuick: false, dockOpen: false,
      links: { USB: true, Tethering: true, 'Wi-Fi': true, Bluetooth: true }
    };
    if (wasLand) setOrientation(false);
    $$('.js-saved-host').forEach(el => el.remove());
    $('m-notifShade').classList.remove('is-open');
    toggleAddForm(false);
    showHelp(mHelpViews, 'bento');
    runBoot();
  };

  $('s-pcReset').onclick = () => {
    clearHudTimer('pc');
    Object.assign(state, {
      pcTrusted: false, pairingRequest: false, userDisconnected: false,
      pcBlocked: false, camera: false, mic: false, speaker: false
    });
    state.sim.pc = { USB: false, Tethering: false, 'Wi-Fi': true, Bluetooth: false };
    state.pc = {
      flyoutOpen: false,
      trayOverflowOpen: true,
      drawer: null, camHud: false, camMenu: false, camHidden: false, micQuick: false, dockOpen: false,
      links: { USB: true, Tethering: true, 'Wi-Fi': true, Bluetooth: true }
    };
    $('p-pairedRow').classList.remove('hidden');
    showHelp(pHelpViews, 'bento');
    runBoot();
  };

  $$('[data-sim-side]').forEach(b => b.onclick = () => {
    const side = b.dataset.simSide, k = b.dataset.simLink;
    state.sim[side][k] = !state.sim[side][k];
    state.booting = false;
    render();
  });

  const toggleMedia = (key) => {
    if (!activeLink()) return;
    state.phone.drawer = state.pc.drawer = null;
    state[key] = !state[key];
    if (key === 'camera' && !state.camera) {
      clearHudTimer('phone'); clearHudTimer('pc');
      state.phone.camHud = state.phone.camMenu = state.pc.camHud = state.pc.camMenu = false;
    }
    if (key === 'mic' && !state.mic) state.phone.micQuick = state.pc.micQuick = false;
    render();
  };

  $('s-hud').onclick = () => {
    if (!activeLink()) return;
    state.phone.drawer = state.pc.drawer = null;
    state.camera = true;
    const next = !(state.phone.camHud || state.pc.camHud);
    state.phone.camHud = state.pc.camHud = next;
    state.phone.camMenu = state.pc.camMenu = false;
    render();
    if (next) { scheduleHudDim('phone'); scheduleHudDim('pc'); }
    else { clearHudTimer('phone'); clearHudTimer('pc'); }
  };

  $('m-primaryRow').onclick = e => {
    if (e.target.closest('button')) return;
    if ($('m-primaryRow').dataset.rowConnect === '1') initiateFromMobile();
  };

  document.addEventListener('click', e => {
    const btn = e.target.closest('[data-conn-act]');
    if (!btn) return;
    const act = btn.dataset.connAct, side = btn.dataset.side;
    if (act === 'connect' && side === 'phone') initiateFromMobile();
    else if (act === 'pair' && side === 'pc') pairOnPc();
    else if (act === 'cancel' || act === 'decline') cancelOrDeclineRequest();
    else if (act === 'disconnect') disconnectSession();
    else if (act === 'forget' && side === 'pc') forgetOnPc();
    else if (act === 'block') {
      if (activeLink()) state.lastConnectedAt = Date.now();
      if (side === 'phone') state.phoneBlocked = true; else state.pcBlocked = true;
      state.pairingRequest = false;
      render();
    } else if (act === 'unblock') {
      if (side === 'phone') state.phoneBlocked = false; else state.pcBlocked = false;
      if (activeLink()) triggerSyncFlash();
      render();
    }
  });

  $$('[data-app-side]').forEach(b => b.onclick = () => {
    const side = b.dataset.appSide, k = b.dataset.transport;
    state[side].links[k] = !state[side].links[k];
    render();
  });

  const bindRefresh = (id) => {
    $(id).onclick = () => {
      $(id).classList.add('is-spinning');
      setTimeout(() => $(id).classList.remove('is-spinning'), 420);
      runBoot();
    };
  };
  bindRefresh('m-btnRefresh');
  bindRefresh('p-btnRefresh');

  $('m-statusBar').onclick = () => { $('m-notifShade').classList.toggle('is-open'); render(); };
  $('m-btnNotifMic').onclick = () => { if (activeLink()) toggleMedia('mic'); };
  $('m-btnNotifStop').onclick = () => {
    clearHudTimer('phone'); clearHudTimer('pc');
    Object.assign(state, { camera: false, mic: false, speaker: false });
    state.phone.camHud = state.phone.camMenu = state.pc.camHud = state.pc.camMenu = false;
    $('m-notifShade').classList.remove('is-open');
    render();
  };

  $('m-btnHelp').onclick = () => { $('m-notifShade').classList.remove('is-open'); state.phone.drawer = state.phone.drawer === 'help' ? null : 'help'; if (state.phone.drawer === 'help') showHelp(mHelpViews, 'bento'); render(); };
  $('m-btnWifi').onclick = () => { $('m-notifShade').classList.remove('is-open'); state.phone.drawer = state.phone.drawer === 'wifi' ? null : 'wifi'; if (state.phone.drawer !== 'wifi') toggleAddForm(false); render(); };
  $('m-btnCam').onclick = () => { state.phone.drawer = null; toggleMedia('camera'); };
  $('m-btnMic').onclick = () => { state.phone.drawer = null; toggleMedia('mic'); };
  $('m-btnSpk').onclick = () => { state.phone.drawer = null; toggleMedia('speaker'); };

  $('btnTrayChevron').onclick = e => {
    e.stopPropagation();
    state.pc.trayOverflowOpen = !state.pc.trayOverflowOpen;
    render();
  };

  $('btnTrayOwl').onclick = e => {
    e.stopPropagation();
    state.pc.flyoutOpen = !state.pc.flyoutOpen;
    render();
  };

  document.addEventListener('pointerdown', e => {
    const inPc = e.target.closest('#pcFlyoutAnchor, #btnTrayOwl, #sandboxPanel, #deviceMobile');
    const inTray = e.target.closest('#trayOverflowFlyout, #btnTrayChevron');

    let changed = false;
    if (state.pc.flyoutOpen && !inPc && !inTray) {
      state.pc.flyoutOpen = false;
      changed = true;
    }
    if (state.pc.trayOverflowOpen && !inTray && !inPc) {
      state.pc.trayOverflowOpen = false;
      changed = true;
    }
    let closedMenuOnDown = false;
    if (state.phone.camMenu && !e.target.closest('#m-camMenuBox')) {
      state.phone.camMenu = false;
      scheduleHudDim('phone');
      closedMenuOnDown = true;
      changed = true;
    }
    if (state.pc.camMenu && !e.target.closest('#p-camMenuBox')) {
      state.pc.camMenu = false;
      scheduleHudDim('pc');
      closedMenuOnDown = true;
      changed = true;
    }
    if (closedMenuOnDown) {
      document.documentElement.dataset.closedCamMenu = '1';
      setTimeout(() => { delete document.documentElement.dataset.closedCamMenu; }, 0);
    }
    if (changed) render();
  });

  $('p-btnWifi').onclick = () => { state.pc.drawer = state.pc.drawer === 'wifi' ? null : 'wifi'; render(); };
  $('p-btnHelp').onclick = () => { state.pc.drawer = state.pc.drawer === 'help' ? null : 'help'; if (state.pc.drawer === 'help') showHelp(pHelpViews, 'bento'); render(); };
  $('p-btnCam').onclick = () => { state.pc.drawer = null; toggleMedia('camera'); };
  $('p-btnMic').onclick = () => { state.pc.drawer = null; toggleMedia('mic'); };
  $('p-btnSpk').onclick = () => { state.pc.drawer = null; toggleMedia('speaker'); };

  $('p-btnRemovePaired').onclick = () => $('p-pairedRow').classList.add('hidden');
  $('p-btnCopyIp').onclick = () => {
    navigator.clipboard?.writeText($('p-hostIp').textContent).catch(() => {});
    clearTimeout(copyTimer);
    $('p-btnCopyIp').innerHTML = iconImg('check', 'w-3.5 h-3.5 owl-icon text-white', 'Copied');
    bindAllFallbacks($('p-btnCopyIp'));
    $('p-btnCopyIp').classList.add('text-white');
    copyTimer = setTimeout(() => {
      $('p-btnCopyIp').innerHTML = iconImg('copy', 'w-3.5 h-3.5 owl-icon', 'Copy');
      bindAllFallbacks($('p-btnCopyIp'));
      $('p-btnCopyIp').classList.remove('text-white');
    }, 1400);
  };

  // Camera HUD & Waveform Controls
  bindRatioBtns = (side, pfx) => {
    const dev = state[side];
    $$(`.js-${pfx}-ratio`).forEach(b => b.onclick = e => {
      e.stopPropagation();
      if (state.ratio === b.dataset.ratio) dev.camMenu = false;
      else state.ratio = b.dataset.ratio;
      dev.camHud = true;
      scheduleHudDim(side);
      render();
    });
  };
  const bindCamUi = (side, pfx) => {
    const dev = state[side];
    $(`${pfx}-camLayer`).onclick = e => {
      if (e.target.closest(`#${pfx}-camHud`)) return;
      if (document.documentElement.dataset.closedCamMenu) return;
      if (dev.camMenu) { dev.camMenu = false; dev.camHud = true; render(); scheduleHudDim(side); }
      else { dev.camHud = !dev.camHud; render(); dev.camHud ? scheduleHudDim(side) : clearHudTimer(side); }
    };
    $(`${pfx}-btnCamMenu`).onclick = () => {
      dev.camHud = true; dev.camMenu = !dev.camMenu;
      if (dev.camMenu) dev.dockOpen = false;
      scheduleHudDim(side);
      render();
    };
    bindRatioBtns(side, pfx);
    $(`${pfx}-btnCamOrient`).onclick = e => {
      e.stopPropagation();
      const nextLand = !state.phone.isLandscape;
      dev.camHud = true;
      scheduleHudDim(side);
      setOrientation(nextLand);
    };
    $(`${pfx}-btnPreview`).onclick = () => {
      dev.camHud = true; dev.camMenu = false; dev.camHidden = !dev.camHidden; render(); scheduleHudDim(side);
    };
    $(`${pfx}-btnMirror`).onclick = () => {
      dev.camHud = true; dev.camMenu = false; state.mirror = !state.mirror; render(); scheduleHudDim(side);
    };
    $(`${pfx}-btnFlip`).onclick = () => {
      dev.camHud = true; dev.camMenu = false; state.lens = state.lens === 'Front' ? 'Back' : 'Front'; render(); scheduleHudDim(side);
    };
    $(`${pfx}-waveHeader`).onclick = () => {
      if (!state.mic) return;
      if (state.camera) {
        dev.dockOpen = !dev.dockOpen;
        if (dev.dockOpen) { dev.camMenu = false; clearHudTimer(side); } else scheduleHudDim(side);
      } else dev.micQuick = !dev.micQuick;
      render();
    };
    $(`${pfx}-playground`).onclick = e => {
      if (state.mic && !state.camera && !dev.drawer && !e.target.closest(`#${pfx}-waveDock, #${pfx}-micQuick`)) {
        dev.micQuick = !dev.micQuick; render();
      }
    };
  };
  bindCamUi('phone', 'm');
  bindCamUi('pc', 'p');

  $('m-btnShowAdd').onclick = () => {
    const isOpen = !$('m-addHostForm').classList.contains('hidden');
    toggleAddForm(!isOpen);
  };
  $('m-btnCancelAdd').onclick = () => toggleAddForm(false);
  const addHost = () => {
    const val = $('m-addPcInput').value.trim();
    if (!val) return;
    const clean = val.toLowerCase();
    if (clean === 'yash-pc' || clean.startsWith('192.168.1.15')) {
      toggleAddForm(false);
      initiateFromMobile();
      return;
    }
    const row = document.createElement('div');
    row.className = 'js-saved-host border-b border-white/10 px-4 py-3.5 flex items-center gap-3.5';
    row.innerHTML =
      `<span class="w-5 h-5 flex items-center justify-center shrink-0 opacity-25">${iconImg('globe', 'w-[19px] h-[19px] owl-img', 'Link')}</span>` +
      `<div class="min-w-0 flex-1"><div class="text-[14px] font-medium text-white/50 leading-snug truncate">${val}</div><div class="text-[12px] text-white/35 leading-snug mt-0.5 truncate">Saved</div></div>` +
      `<button class="py-1 text-[12px] text-white/35 active:text-white/70 shrink-0 cursor-pointer">Remove</button>`;
    bindAllFallbacks(row);
    row.querySelector('button').onclick = () => row.remove();
    $('m-addHostForm').insertAdjacentElement('beforebegin', row);
    toggleAddForm(false);
  };
  $('m-btnAddPc').onclick = addHost;
  $('m-addPcInput').onkeydown = e => { if (e.key === 'Enter') addHost(); if (e.key === 'Escape') toggleAddForm(false); };

  $$('[data-m-help]').forEach(el => el.onclick = () => showHelp(mHelpViews, el.dataset.mHelp));
  $$('[data-m-help-back]').forEach(b => b.onclick = () => showHelp(mHelpViews, 'bento'));
  $$('[data-p-help]').forEach(el => el.onclick = () => showHelp(pHelpViews, el.dataset.pHelp));
  $$('[data-p-help-back]').forEach(b => b.onclick = () => showHelp(pHelpViews, 'bento'));

  $$('.js-nr-sl').forEach(s => s.oninput = e => { state.nr = +e.target.value; syncAudio(); });
  $$('.js-vol-sl').forEach(s => s.oninput = e => { state.vol = +e.target.value; syncAudio(); });
  $$('.js-iso-sw').forEach(sw => sw.onclick = () => { state.voiceIso = !state.voiceIso; syncAudio(); });
  $$('.js-norm-sw').forEach(sw => sw.onclick = () => { state.normalization = !state.normalization; syncAudio(); });

  const tickClock = () => {
    const d = new Date();
    const hm = `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`;
    $('m-statusTime').textContent = $('m-lockClock').textContent = hm;

    let hours = d.getHours();
    const ampm = hours >= 12 ? 'PM' : 'AM';
    hours = hours % 12 || 12;
    const trayHm = `${hours}:${String(d.getMinutes()).padStart(2, '0')} ${ampm}`;
    $('trayTime').textContent = trayHm;
    $('trayDate').textContent = `${d.getMonth() + 1}/${d.getDate()}/${d.getFullYear()}`;
    if (state.lastConnectedAt && !activeLink()) render();
  };
  tickClock(); setInterval(tickClock, 15000);

  syncAudio();
  syncMobilePg();
  fitPhoneStage();
  runBoot();
})();

