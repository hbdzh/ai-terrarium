export interface AvatarSpec {
  bg: string;
  fg: string;
  accent: string;
  headShape: "rounded" | "circle" | "hex" | "squircle";
  eyes: "dots" | "visor" | "happy" | "wink";
  detail: "antenna" | "halo" | "ears" | "none";
}

const PALETTES: Array<{ bg: string; fg: string; accent: string }> = [
  { bg: "#EAF0FF", fg: "#4B6CB7", accent: "#8FA8E8" },
  { bg: "#EEE9FA", fg: "#7553A7", accent: "#B89EE3" },
  { bg: "#E6F2ED", fg: "#3F7D67", accent: "#86C5AE" },
  { bg: "#FFF1E6", fg: "#C06732", accent: "#F0A87B" },
  { bg: "#FCEBF1", fg: "#B64D77", accent: "#E79AB8" },
  { bg: "#E7F4FA", fg: "#367A9E", accent: "#83C3E3" },
];

const HEAD_SHAPES: AvatarSpec["headShape"][] = ["rounded", "circle", "hex", "squircle"];
const EYE_STYLES: AvatarSpec["eyes"][] = ["dots", "visor", "happy", "wink"];
const DETAILS: AvatarSpec["detail"][] = ["antenna", "halo", "ears", "none"];

export function seedFromString(str: string): number {
  let h = 2166136261;
  for (let i = 0; i < str.length; i += 1) {
    h ^= str.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  return Math.abs(h) || 1;
}

export function randomAvatarSeed(): number {
  return Math.floor(1 + Math.random() * 1_000_000_000);
}

export function getAvatarSpec(seed: number): AvatarSpec {
  const s = Math.abs(Math.floor(seed)) || 1;
  const pal = PALETTES[s % PALETTES.length];
  return {
    bg: pal.bg,
    fg: pal.fg,
    accent: pal.accent,
    headShape: HEAD_SHAPES[(s >> 3) % HEAD_SHAPES.length],
    eyes: EYE_STYLES[(s >> 6) % EYE_STYLES.length],
    detail: DETAILS[(s >> 9) % DETAILS.length],
  };
}
