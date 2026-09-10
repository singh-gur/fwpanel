// Strict mirrors of the Rust DTOs in crates/fwpanel-protocol/src/lib.rs.
// Keep both sides in sync; snake_case matches the wire format exactly.

export interface ServiceInfo {
  service_version: string;
  protocol_version: number;
  library_version: string;
  features: string[];
}

export interface Battery {
  percentage: number;
  charging: boolean;
  discharging: boolean;
  critical: boolean;
  remaining_capacity_mah: number;
  last_full_charge_capacity_mah: number;
  design_capacity_mah: number;
  voltage_mv: number;
  cycle_count: number;
}

export interface ChargeLimits {
  minimum_percent: number;
  maximum_percent: number;
}

export type ChargeLimitReading =
  | { status: "ok"; limits: ChargeLimits }
  | { status: "failed"; message: string };

export interface PowerSnapshot {
  timestamp_ms: number;
  ac_present: boolean;
  battery: Battery | null;
  charge_limit: ChargeLimitReading;
}
