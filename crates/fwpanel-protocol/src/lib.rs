//! Shared wire types for the fwpanel system-service protocol.
//!
//! Every D-Bus method of `io.github.singh_gur.Fwpanel1` returns a JSON string
//! encoding a [`Reply`]. The GUI backend decodes replies with [`Reply::decode`],
//! which enforces message size, protocol compatibility, and shape before any
//! typed value reaches the UI. Unknown *fields* are ignored (additive changes
//! within protocol major 1 stay compatible); unknown enum variants or a
//! different `protocol_version` are hard errors.

use serde::{de::DeserializeOwned, Deserialize, Serialize};

/// Major version of the fwpanel service protocol.
pub const PROTOCOL_MAJOR: u32 = 1;

/// Maximum accepted size of a single service reply, in bytes.
pub const MAX_MESSAGE_BYTES: usize = 64 * 1024;

/// Lowest charge-limit maximum the UI and service accept (upstream CLI range).
pub const CHARGE_LIMIT_MIN_REQUEST: u32 = 25;
/// Highest charge-limit maximum the UI and service accept.
pub const CHARGE_LIMIT_MAX_REQUEST: u32 = 100;

/// Feature names advertised by [`ServiceInfo`]. An advertised feature is a
/// claim of implementation, not of firmware support.
pub mod feature {
    pub const BATTERY: &str = "battery";
    pub const CHARGE_LIMIT_READ: &str = "charge_limit_read";
    pub const CHARGE_LIMIT_WRITE: &str = "charge_limit_write";
    pub const PORTS: &str = "ports";
    pub const INPUT_DECK: &str = "input_deck";
}

/// Stable error codes carried by error replies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    AccessDenied,
    UnsupportedPlatform,
    DriverUnavailable,
    UnsupportedFeature,
    HardwareUnavailable,
    InvalidData,
    InvalidArgument,
    Busy,
    OutcomeUnknown,
}

impl ErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            ErrorCode::AccessDenied => "access_denied",
            ErrorCode::UnsupportedPlatform => "unsupported_platform",
            ErrorCode::DriverUnavailable => "driver_unavailable",
            ErrorCode::UnsupportedFeature => "unsupported_feature",
            ErrorCode::HardwareUnavailable => "hardware_unavailable",
            ErrorCode::InvalidData => "invalid_data",
            ErrorCode::InvalidArgument => "invalid_argument",
            ErrorCode::Busy => "busy",
            ErrorCode::OutcomeUnknown => "outcome_unknown",
        }
    }
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Envelope for every method reply: tagged `ok` or `error`, always carrying
/// the protocol version of the service.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Reply<T> {
    Ok {
        protocol_version: u32,
        data: T,
    },
    Error {
        protocol_version: u32,
        code: ErrorCode,
        message: String,
    },
}

impl<T: Serialize> Reply<T> {
    /// Encode a success reply. Only fails if `data` cannot be serialized,
    /// which is a programming error in the service.
    pub fn ok_json(data: &T) -> Result<String, serde_json::Error> {
        serde_json::to_string(&Reply::Ok {
            protocol_version: PROTOCOL_MAJOR,
            data,
        })
    }

    /// Encode an error reply with a safe human-readable message.
    pub fn error_json(code: ErrorCode, message: &str) -> Result<String, serde_json::Error> {
        serde_json::to_string(&Reply::<()>::Error {
            protocol_version: PROTOCOL_MAJOR,
            code,
            message: message.to_string(),
        })
    }
}

/// Client-side reply decoding failures, each with a clear user-facing cause.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplyError {
    Oversized,
    Malformed(String),
    Incompatible { reply_protocol: u32 },
    Service { code: ErrorCode, message: String },
}

impl std::fmt::Display for ReplyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReplyError::Oversized => write!(
                f,
                "fwpanel service reply exceeded the {MAX_MESSAGE_BYTES}-byte limit"
            ),
            ReplyError::Malformed(reason) => {
                write!(f, "invalid reply from the fwpanel service: {reason}")
            }
            ReplyError::Incompatible { reply_protocol } => write!(
                f,
                "the fwpanel service speaks protocol version {reply_protocol}, \
                 this app expects {PROTOCOL_MAJOR}; update fwpanel-service or the app"
            ),
            ReplyError::Service { code, message } => {
                write!(f, "fwpanel service reported an error ({code}): {message}")
            }
        }
    }
}

impl std::error::Error for ReplyError {}

impl<T: DeserializeOwned> Reply<T> {
    /// Decode a JSON reply string into the method's typed data, enforcing the
    /// size limit, shape, and protocol compatibility before returning anything.
    pub fn decode(json: &str) -> Result<T, ReplyError> {
        if json.len() > MAX_MESSAGE_BYTES {
            return Err(ReplyError::Oversized);
        }
        let reply: Reply<T> =
            serde_json::from_str(json).map_err(|e| ReplyError::Malformed(e.to_string()))?;
        match reply {
            Reply::Ok {
                protocol_version,
                data,
            } => {
                if protocol_version != PROTOCOL_MAJOR {
                    return Err(ReplyError::Incompatible {
                        reply_protocol: protocol_version,
                    });
                }
                Ok(data)
            }
            Reply::Error {
                protocol_version,
                code,
                message,
            } => {
                if protocol_version != PROTOCOL_MAJOR {
                    return Err(ReplyError::Incompatible {
                        reply_protocol: protocol_version,
                    });
                }
                Err(ReplyError::Service { code, message })
            }
        }
    }
}

/// Why a typed value failed validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationError {
    /// A caller-supplied value is out of range.
    InvalidArgument,
    /// A value observed from hardware is impossible and must not be shown.
    InvalidData,
}

impl ValidationError {
    pub const fn error_code(self) -> ErrorCode {
        match self {
            ValidationError::InvalidArgument => ErrorCode::InvalidArgument,
            ValidationError::InvalidData => ErrorCode::InvalidData,
        }
    }
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            ValidationError::InvalidArgument => "value out of range",
            ValidationError::InvalidData => "implausible value read from hardware",
        })
    }
}

/// Validate a user-supplied charge-limit maximum request.
pub fn validate_charge_limit_request(maximum: u32) -> Result<u8, ValidationError> {
    if (CHARGE_LIMIT_MIN_REQUEST..=CHARGE_LIMIT_MAX_REQUEST).contains(&maximum) {
        Ok(maximum as u8)
    } else {
        Err(ValidationError::InvalidArgument)
    }
}

/// Static service description. Does not probe hardware and exposes no serial
/// numbers or identifiers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceInfo {
    pub service_version: String,
    pub protocol_version: u32,
    pub library_version: String,
    pub features: Vec<String>,
}

/// Battery readings. Capacities are mAh, voltage is mV. Manufacturer, model,
/// and serial strings deliberately do not cross this boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Battery {
    pub percentage: u8,
    pub charging: bool,
    pub discharging: bool,
    pub critical: bool,
    pub remaining_capacity_mah: u32,
    pub last_full_charge_capacity_mah: u32,
    pub design_capacity_mah: u32,
    pub voltage_mv: u32,
    pub cycle_count: u32,
}

impl Battery {
    /// Reject implausible readings instead of clamping them.
    pub fn validated(self) -> Result<Self, ValidationError> {
        if self.percentage > 100 {
            return Err(ValidationError::InvalidData);
        }
        Ok(self)
    }
}

/// Charge-limit percentages as reported by the EC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChargeLimits {
    pub minimum_percent: u8,
    pub maximum_percent: u8,
}

impl ChargeLimits {
    /// Reject out-of-range or sentinel values (e.g. `0xFF`) and min > max.
    pub fn validated(self) -> Result<Self, ValidationError> {
        if self.minimum_percent > 100
            || self.maximum_percent > 100
            || self.minimum_percent > self.maximum_percent
        {
            return Err(ValidationError::InvalidData);
        }
        Ok(self)
    }
}

/// Charge-limit reading inside a power snapshot; it can fail independently of
/// the battery data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ChargeLimitReading {
    Ok { limits: ChargeLimits },
    Failed { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PowerSnapshot {
    /// Unix epoch milliseconds at sample time.
    pub timestamp_ms: u64,
    pub ac_present: bool,
    /// `None` only when a successful read confirms no battery.
    pub battery: Option<Battery>,
    pub charge_limit: ChargeLimitReading,
}

/// USB-C power role (upstream `UsbPowerRoles`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortRole {
    Disconnected,
    Source,
    Sink,
    SinkNotCharging,
}

/// USB-C charging type (upstream `UsbChargingType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChargingType {
    None,
    Pd,
    TypeC,
    Proprietary,
    Bc12Dcp,
    Bc12Cdp,
    Bc12Sdp,
    Other,
    VBus,
    Unknown,
}

/// USB-C power status of one indexed port. Voltages mV, currents mA, power mW.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Port {
    /// Laptop 13 mapping: 0 right rear, 1 right front, 2 left front, 3 left rear.
    pub index: u8,
    pub role: PortRole,
    pub charging_type: ChargingType,
    pub current_voltage_mv: u32,
    pub max_voltage_mv: u32,
    pub current_limit_ma: u32,
    pub max_current_ma: u32,
    pub dual_role: bool,
    pub max_power_mw: u32,
}

impl Port {
    pub fn validated(self) -> Result<Self, ValidationError> {
        if self.index > 3 {
            return Err(ValidationError::InvalidData);
        }
        Ok(self)
    }
}

/// Per-port result: a failed port is independently unavailable, never absent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum PortResult {
    Ok { port: Port },
    Unavailable { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortsSnapshot {
    pub timestamp_ms: u64,
    pub ports: [PortResult; 4],
}

/// Input-deck power state (upstream `InputDeckState`). Laptop 13 only; no
/// Laptop 16 slot layout is exposed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeckState {
    Off,
    Disconnected,
    TurningOn,
    On,
    ForceOff,
    ForceOn,
    NoDetection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputDeckSnapshot {
    pub timestamp_ms: u64,
    pub deck_state: DeckState,
    pub touchpad_present: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn service_info() -> ServiceInfo {
        ServiceInfo {
            service_version: "0.1.0".into(),
            protocol_version: PROTOCOL_MAJOR,
            library_version: "0.6.5".into(),
            features: vec![],
        }
    }

    fn power_snapshot() -> PowerSnapshot {
        PowerSnapshot {
            timestamp_ms: 1_700_000_000_000,
            ac_present: true,
            battery: Some(Battery {
                percentage: 80,
                charging: true,
                discharging: false,
                critical: false,
                remaining_capacity_mah: 4800,
                last_full_charge_capacity_mah: 6000,
                design_capacity_mah: 6200,
                voltage_mv: 12_800,
                cycle_count: 42,
            }),
            charge_limit: ChargeLimitReading::Ok {
                limits: ChargeLimits {
                    minimum_percent: 40,
                    maximum_percent: 80,
                },
            },
        }
    }

    #[test]
    fn ok_reply_round_trip() {
        let json = Reply::ok_json(&service_info()).unwrap();
        assert_eq!(Reply::<ServiceInfo>::decode(&json).unwrap(), service_info());

        let json = Reply::ok_json(&power_snapshot()).unwrap();
        assert_eq!(
            Reply::<PowerSnapshot>::decode(&json).unwrap(),
            power_snapshot()
        );
    }

    #[test]
    fn error_codes_round_trip() {
        for code in [
            ErrorCode::AccessDenied,
            ErrorCode::UnsupportedPlatform,
            ErrorCode::DriverUnavailable,
            ErrorCode::UnsupportedFeature,
            ErrorCode::HardwareUnavailable,
            ErrorCode::InvalidData,
            ErrorCode::InvalidArgument,
            ErrorCode::Busy,
            ErrorCode::OutcomeUnknown,
        ] {
            let json = Reply::<()>::error_json(code, "msg").unwrap();
            assert_eq!(
                Reply::<ServiceInfo>::decode(&json),
                Err(ReplyError::Service {
                    code,
                    message: "msg".into()
                })
            );
            // Wire form uses the stable snake_case code string.
            assert!(json.contains(&format!("\"{}\"", code.as_str())));
        }
    }

    #[test]
    fn malformed_replies_are_rejected() {
        // Truncated JSON.
        assert!(matches!(
            Reply::<ServiceInfo>::decode("{\"status\""),
            Err(ReplyError::Malformed(_))
        ));
        // Wrong data shape.
        assert!(matches!(
            Reply::<ServiceInfo>::decode(
                r#"{"status":"ok","protocol_version":1,"data":{"features":"not-a-list"}}"#
            ),
            Err(ReplyError::Malformed(_))
        ));
        // Unknown error code.
        assert!(matches!(
            Reply::<ServiceInfo>::decode(
                r#"{"status":"error","protocol_version":1,"code":"quantum","message":"x"}"#
            ),
            Err(ReplyError::Malformed(_))
        ));
        // Unknown mandatory enum tag inside data.
        assert!(matches!(
            Reply::<PowerSnapshot>::decode(
                r#"{"status":"ok","protocol_version":1,"data":{"timestamp_ms":1,
                   "ac_present":false,"battery":null,
                   "charge_limit":{"status":"maybe","limits":{"minimum_percent":0,
                   "maximum_percent":100}}}}"#
                    .replace('\n', "")
                    .as_str()
            ),
            Err(ReplyError::Malformed(_))
        ));
        // Unknown tag.
        assert!(matches!(
            Reply::<ServiceInfo>::decode(r#"{"status":"maybe","protocol_version":1}"#),
            Err(ReplyError::Malformed(_))
        ));
    }

    #[test]
    fn oversized_reply_is_rejected() {
        let big = format!(
            r#"{{"status":"error","protocol_version":1,"code":"busy","message":"{}"}}"#,
            "x".repeat(MAX_MESSAGE_BYTES)
        );
        assert_eq!(Reply::<()>::decode(&big), Err(ReplyError::Oversized));
    }

    #[test]
    fn protocol_version_mismatch_is_incompatible() {
        let json = r#"{"status":"ok","protocol_version":2,"data":{"service_version":"x",
               "protocol_version":2,"library_version":"y","features":[]}}"#
            .replace('\n', "");
        assert_eq!(
            Reply::<ServiceInfo>::decode(&json),
            Err(ReplyError::Incompatible { reply_protocol: 2 })
        );
    }

    #[test]
    fn additive_fields_are_ignored() {
        let json = r#"{"status":"ok","protocol_version":1,"data":{"service_version":"x",
            "protocol_version":1,"library_version":"y","features":[],"future_field":true}}"#
            .replace('\n', "");
        assert!(Reply::<ServiceInfo>::decode(&json).is_ok());
    }

    #[test]
    fn error_reply_with_wrong_protocol_version_is_incompatible() {
        let json = r#"{"status":"error","protocol_version":2,"code":"busy","message":"x"}"#;
        assert_eq!(
            Reply::<ServiceInfo>::decode(json),
            Err(ReplyError::Incompatible { reply_protocol: 2 })
        );
    }

    #[test]
    fn charge_limit_requests_are_validated() {
        assert_eq!(validate_charge_limit_request(25), Ok(25));
        assert_eq!(validate_charge_limit_request(100), Ok(100));
        assert_eq!(
            validate_charge_limit_request(24),
            Err(ValidationError::InvalidArgument)
        );
        assert_eq!(
            validate_charge_limit_request(101),
            Err(ValidationError::InvalidArgument)
        );
        assert_eq!(
            validate_charge_limit_request(u32::MAX),
            Err(ValidationError::InvalidArgument)
        );
    }

    #[test]
    fn observed_charge_limits_are_validated() {
        let ok = ChargeLimits {
            minimum_percent: 40,
            maximum_percent: 80,
        };
        assert_eq!(ok.validated(), Ok(ok));
        // Sentinel minimum from the EC (0xFF) must be rejected, not clamped.
        let sentinel = ChargeLimits {
            minimum_percent: 255,
            maximum_percent: 80,
        };
        assert_eq!(sentinel.validated(), Err(ValidationError::InvalidData));
        let inverted = ChargeLimits {
            minimum_percent: 80,
            maximum_percent: 40,
        };
        assert_eq!(inverted.validated(), Err(ValidationError::InvalidData));
    }

    #[test]
    fn implausible_battery_is_rejected() {
        let mut battery = power_snapshot().battery.unwrap();
        battery.percentage = 101;
        assert_eq!(battery.validated(), Err(ValidationError::InvalidData));
    }

    #[test]
    fn port_index_is_validated() {
        let mut port = match ports_snapshot().ports[0].clone() {
            PortResult::Ok { port } => port,
            other => panic!("expected ok port, got {other:?}"),
        };
        port.index = 4;
        assert_eq!(port.validated(), Err(ValidationError::InvalidData));
    }

    fn ports_snapshot() -> PortsSnapshot {
        let port = |index: u8, role: PortRole| PortResult::Ok {
            port: Port {
                index,
                role,
                charging_type: ChargingType::Pd,
                current_voltage_mv: 20_000,
                max_voltage_mv: 20_000,
                current_limit_ma: 3_250,
                max_current_ma: 5_000,
                dual_role: true,
                max_power_mw: 65_000,
            },
        };
        PortsSnapshot {
            timestamp_ms: 1_700_000_000_000,
            ports: [
                port(0, PortRole::Sink),
                PortResult::Unavailable {
                    message: "port read failed".into(),
                },
                port(2, PortRole::Disconnected),
                port(3, PortRole::Source),
            ],
        }
    }

    #[test]
    fn ports_snapshot_round_trip_preserves_independent_failures() {
        let json = Reply::ok_json(&ports_snapshot()).unwrap();
        assert_eq!(
            Reply::<PortsSnapshot>::decode(&json).unwrap(),
            ports_snapshot()
        );
    }

    #[test]
    fn input_deck_snapshot_round_trip() {
        let snapshot = InputDeckSnapshot {
            timestamp_ms: 1_700_000_000_000,
            deck_state: DeckState::On,
            touchpad_present: true,
        };
        let json = Reply::ok_json(&snapshot).unwrap();
        assert_eq!(Reply::<InputDeckSnapshot>::decode(&json).unwrap(), snapshot);
    }
}
