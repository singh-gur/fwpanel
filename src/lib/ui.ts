// Shared presentation-layer types. Keeps the icon/tone vocabularies in one
// place so components stay consistent (and so .svelte files never have to
// re-export types).

export type IconName =
  | "battery"
  | "bolt"
  | "plug"
  | "gauge"
  | "keyboard"
  | "refresh"
  | "check"
  | "alert"
  | "shield"
  | "chip"
  | "arrow-down"
  | "arrow-up";

export type Tone = "neutral" | "accent" | "ok" | "warn" | "danger" | "info";
