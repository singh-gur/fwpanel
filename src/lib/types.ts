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

export type PortRole =
  | "disconnected"
  | "source"
  | "sink"
  | "sink_not_charging";

export type ChargingType =
  | "none"
  | "pd"
  | "type_c"
  | "proprietary"
  | "bc12_dcp"
  | "bc12_cdp"
  | "bc12_sdp"
  | "other"
  | "vbus"
  | "unknown";

export interface Port {
  index: number;
  role: PortRole;
  charging_type: ChargingType;
  current_voltage_mv: number;
  max_voltage_mv: number;
  current_limit_ma: number;
  max_current_ma: number;
  dual_role: boolean;
  max_power_mw: number;
}

export type PortResult =
  | { status: "ok"; port: Port }
  | { status: "unavailable"; message: string };

export interface PortsSnapshot {
  timestamp_ms: number;
  ports: PortResult[]; // always 4 entries
}

export type DeckState =
  | "off"
  | "disconnected"
  | "turning_on"
  | "on"
  | "force_off"
  | "force_on"
  | "no_detection";

export interface InputDeckSnapshot {
  timestamp_ms: number;
  deck_state: DeckState;
  touchpad_present: boolean;
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
