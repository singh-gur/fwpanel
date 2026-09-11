// Display formatting for hardware readings. The service reports raw integer
// milli-units; the UI should never show `17.048 V` or `4732 mAh` unformatted.

const NUM = new Intl.NumberFormat();

/** Thousands-separated integer, e.g. 4732 → "4,732". */
export function int(value: number): string {
  return NUM.format(Math.round(value));
}

/** Millivolts → volts with two decimals, e.g. 17048 → "17.05 V". */
export function volts(mv: number): string {
  return `${(mv / 1000).toFixed(2)} V`;
}

/** Milliamps → A above 1 A, else mA. */
export function amps(ma: number): string {
  return ma >= 1000 ? `${(ma / 1000).toFixed(2)} A` : `${int(ma)} mA`;
}

/** Milliwatts → W above 1 W, else mW. */
export function watts(mw: number): string {
  return mw >= 1000 ? `${(mw / 1000).toFixed(mw % 1000 === 0 ? 0 : 1)} W` : `${int(mw)} mW`;
}

/** Millivolts → V above 1 V, else mV (port readouts mix both scales). */
export function voltsCompact(mv: number): string {
  return mv >= 1000 ? `${(mv / 1000).toFixed(mv % 1000 === 0 ? 0 : 1)} V` : `${int(mv)} mV`;
}

export function time(date: Date | null): string {
  return date ? date.toLocaleTimeString() : "";
}

/**
 * Remaining capacity as a share of the last full charge, which is what the
 * pack can actually still deliver today.
 */
export function healthPercent(lastFullMah: number, designMah: number): number | null {
  if (designMah <= 0 || lastFullMah <= 0) return null;
  // A fresh pack can report slightly above its design capacity; showing "101%"
  // reads as a glitch, and the raw mAh figures are displayed alongside anyway.
  return Math.min(100, Math.round((lastFullMah / designMah) * 100));
}

/**
 * Strip the machine-facing `service-*:` prefix the Tauri layer attaches, so
 * error text reads as a sentence instead of a wire code.
 */
export function humanError(message: string): string {
  return message.replace(/^service-[a-z-]+:\s*/i, "").trim();
}
