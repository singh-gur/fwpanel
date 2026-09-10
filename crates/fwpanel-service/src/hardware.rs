//! Hardware access boundary: platform/driver selection, serialized gate,
//! and conversion from upstream `framework_lib` types to protocol DTOs.
//!
//! All EC work runs through [`Hardware::run`], which:
//! - executes blocking library calls on a dedicated blocking thread
//!   (never the async executor or a UI thread),
//! - allows one operation at a time (competing callers get `busy`, no
//!   queueing), and
//! - bounds each operation to five seconds. A timed-out or panicking worker
//!   is never cancelled or reused: the service process terminates and systemd
//!   restarts it. This is the approved containment for upstream code that can
//!   panic or hang on unexpected EC data.
//!
//! Nothing here is reachable without the caller having passed polkit read
//! authorization in `main`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use framework_lib::chromium_ec::{CrosEc, CrosEcDriverType, EcResult};
use framework_lib::power::{BatteryInformation, PowerInfo};
use framework_lib::smbios::{self, Platform};
use fwpanel_protocol::{
    feature, Battery, ChargeLimitReading, ChargeLimits, ErrorCode, PowerSnapshot,
};

/// Maximum duration of a single hardware operation.
const HARDWARE_DEADLINE: Duration = Duration::from_secs(5);

/// Failure of a hardware-bound request, with a stable protocol error code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HardwareError {
    pub code: ErrorCode,
    pub message: String,
}

impl HardwareError {
    fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for HardwareError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.message, self.code)
    }
}

/// Platform/driver state resolved once at startup.
pub enum HardwareAccess {
    /// SMBIOS platform is not the supported Framework Laptop 13 AMD AI 300.
    UnsupportedPlatform,
    /// Supported platform, but the kernel cros_ec driver is unavailable.
    DriverUnavailable,
    Ready(CrosEc, Hardware),
}

impl HardwareAccess {
    /// The only platform with hardware operations enabled in this release.
    pub fn detect() -> Self {
        match smbios::get_platform() {
            Some(Platform::Framework13AmdAi300) => match CrosEc::with(CrosEcDriverType::CrosEc) {
                Some(ec) => Self::Ready(ec, Hardware::new()),
                None => Self::DriverUnavailable,
            },
            _ => Self::UnsupportedPlatform,
        }
    }

    fn access(&self) -> Result<(&CrosEc, &Hardware), HardwareError> {
        match self {
            Self::Ready(ec, gate) => Ok((ec, gate)),
            Self::UnsupportedPlatform => Err(HardwareError::new(
                ErrorCode::UnsupportedPlatform,
                "this hardware platform is not supported by fwpanel",
            )),
            Self::DriverUnavailable => Err(HardwareError::new(
                ErrorCode::DriverUnavailable,
                "the kernel cros_ec driver is not available",
            )),
        }
    }

    /// Serve `GetPower`: one serialized read of battery/AC status plus an
    /// independent charge-limit read.
    pub async fn power(&self) -> Result<PowerSnapshot, HardwareError> {
        let (ec, gate) = self.access()?;
        let ec = ec.clone();
        gate.run(move || {
            // Note: `power_info` panics on some malformed EC data upstream
            // (unwrap/index/division) — contained by the process-abort policy.
            let power = framework_lib::power::power_info(&ec);
            let charge = ec.get_charge_limit();
            build_power_snapshot(power, charge)
        })
        .await
        .and_then(|inner| inner)
    }
}

/// Single non-queuing hardware gate.
pub struct Hardware {
    busy: AtomicBool,
}

/// Outcome of a gate attempt, distinguishing why no value was produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateError {
    /// Another hardware operation is in flight.
    Busy,
    /// The operation exceeded its deadline; the worker was not cancelled.
    TimedOut,
    /// The worker panicked; its state cannot be trusted.
    Panicked,
}

impl Default for Hardware {
    fn default() -> Self {
        Self::new()
    }
}

impl Hardware {
    pub fn new() -> Self {
        Self {
            busy: AtomicBool::new(false),
        }
    }

    /// Testable core: run `f` under the gate with `deadline`. On timeout or
    /// worker panic the gate is deliberately left engaged (the worker may
    /// still be interacting with the EC); callers that keep the process alive
    /// must treat this as fatal.
    pub async fn run_timed<T, F>(&self, deadline: Duration, f: F) -> Result<T, GateError>
    where
        T: Send + 'static,
        F: FnOnce() -> T + Send + 'static,
    {
        if self.busy.swap(true, Ordering::SeqCst) {
            return Err(GateError::Busy);
        }

        let handle = tokio::task::spawn_blocking(f);
        let result = match tokio::time::timeout(deadline, handle).await {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(join)) if join.is_panic() => Err(GateError::Panicked),
            Ok(Err(_)) => Err(GateError::Panicked), // cancelled without panic
            Err(_) => Err(GateError::TimedOut),
        };
        if result.is_ok() {
            self.busy.store(false, Ordering::SeqCst);
        }
        result
    }

    /// Production entry point: applies the approved deadline and turns
    /// timeout/panic into process failure instead of releasing the gate.
    pub async fn run<T, F>(&self, f: F) -> Result<T, HardwareError>
    where
        T: Send + 'static,
        F: FnOnce() -> T + Send + 'static,
    {
        match self.run_timed(HARDWARE_DEADLINE, f).await {
            Ok(value) => Ok(value),
            Err(GateError::Busy) => Err(HardwareError::new(
                ErrorCode::Busy,
                "another hardware operation is in progress",
            )),
            Err(GateError::TimedOut) => {
                // The blocking task was NOT cancelled and may still touch the
                // EC. Terminating is the only safe recovery; systemd restarts.
                eprintln!("fwpanel-service: hardware operation timed out; terminating");
                std::process::abort();
            }
            Err(GateError::Panicked) => {
                eprintln!("fwpanel-service: hardware worker panicked; terminating");
                std::process::abort();
            }
        }
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Convert one upstream read into the wire snapshot. The charge-limit failure
/// is preserved independently so it never hides valid battery data.
fn build_power_snapshot(
    power: Option<PowerInfo>,
    charge: EcResult<(u8, u8)>,
) -> Result<PowerSnapshot, HardwareError> {
    let Some(power) = power else {
        // Upstream maps a failed EC memory read to None. Only a successful
        // `PowerInfo` with `battery: None` means "no battery".
        return Err(HardwareError::new(
            ErrorCode::HardwareUnavailable,
            "failed to read power status from the EC",
        ));
    };

    let battery = match &power.battery {
        None => None,
        Some(b) => Some(map_battery(b).map_err(|e| {
            HardwareError::new(
                ErrorCode::InvalidData,
                format!("implausible battery data: {e}"),
            )
        })?),
    };

    let charge_limit = match charge {
        Ok((min, max)) => {
            let limits = ChargeLimits {
                minimum_percent: min,
                maximum_percent: max,
            }
            .validated();
            match limits {
                Ok(limits) => ChargeLimitReading::Ok { limits },
                Err(_) => ChargeLimitReading::Failed {
                    message: "implausible charge limits read from the EC".into(),
                },
            }
        }
        Err(e) => ChargeLimitReading::Failed {
            message: format!("charge limit read failed: {e:?}"),
        },
    };

    Ok(PowerSnapshot {
        timestamp_ms: now_ms(),
        ac_present: power.ac_present,
        battery,
        charge_limit,
    })
}

/// Copy only the approved fields; manufacturer/model/serial stay behind the
/// service boundary. Units are preserved (mAh / mV).
fn map_battery(b: &BatteryInformation) -> Result<Battery, fwpanel_protocol::ValidationError> {
    Battery {
        percentage: u8::try_from(b.charge_percentage)
            .map_err(|_| fwpanel_protocol::ValidationError::InvalidData)?,
        charging: b.charging,
        discharging: b.discharging,
        critical: b.level_critical,
        remaining_capacity_mah: b.remaining_capacity,
        last_full_charge_capacity_mah: b.last_full_charge_capacity,
        design_capacity_mah: b.design_capacity,
        voltage_mv: b.present_voltage,
        cycle_count: b.cycle_count,
    }
    .validated() // rejects percentage > 100 instead of clamping
}

/// Features implemented by this build, advertised through `ServiceInfo`.
pub fn implemented_features() -> Vec<String> {
    vec![
        feature::BATTERY.to_string(),
        feature::CHARGE_LIMIT_READ.to_string(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn battery(percentage: u32) -> BatteryInformation {
        BatteryInformation {
            present_voltage: 12_800,
            present_rate: 2_500,
            remaining_capacity: 4_800,
            battery_count: 1,
            current_battery_index: 0,
            design_capacity: 6_200,
            design_voltage: 13_000,
            last_full_charge_capacity: 6_000,
            cycle_count: 42,
            charge_percentage: percentage,
            manufacturer: "DO-NOT-LEAK".into(),
            model_number: "DO-NOT-LEAK".into(),
            serial_number: "DO-NOT-LEAK".into(),
            battery_type: "DO-NOT-LEAK".into(),
            discharging: true,
            charging: false,
            level_critical: false,
        }
    }

    fn power(ac: bool, battery: Option<BatteryInformation>) -> Option<PowerInfo> {
        Some(PowerInfo {
            ac_present: ac,
            battery,
        })
    }

    #[test]
    fn absent_battery_only_from_successful_read() {
        let snap = build_power_snapshot(power(true, None), Ok((40, 80))).unwrap();
        assert!(snap.ac_present);
        assert!(snap.battery.is_none());
        assert!(matches!(snap.charge_limit, ChargeLimitReading::Ok { .. }));
    }

    #[test]
    fn failed_ec_read_is_unavailable_not_absent_battery() {
        let err = build_power_snapshot(None, Ok((40, 80))).unwrap_err();
        assert_eq!(err.code, ErrorCode::HardwareUnavailable);
    }

    #[test]
    fn flags_map_through() {
        let mut b = battery(55);
        b.charging = true;
        b.discharging = false;
        b.level_critical = true;
        let snap = build_power_snapshot(power(false, Some(b)), Ok((40, 80))).unwrap();
        let battery = snap.battery.unwrap();
        assert!(battery.charging && !battery.discharging && battery.critical);
        assert!(!snap.ac_present);
    }

    #[test]
    fn charge_limit_failure_does_not_hide_battery() {
        let snap = build_power_snapshot(
            power(true, Some(battery(80))),
            Err(framework_lib::chromium_ec::EcError::DeviceError(
                "boom".into(),
            )),
        )
        .unwrap();
        assert!(snap.battery.is_some());
        assert!(matches!(
            snap.charge_limit,
            ChargeLimitReading::Failed { .. }
        ));
    }

    #[test]
    fn sentinel_limits_become_failed_reading_not_clamped() {
        let snap = build_power_snapshot(power(true, Some(battery(80))), Ok((0xFF, 80))).unwrap();
        assert!(matches!(
            snap.charge_limit,
            ChargeLimitReading::Failed { .. }
        ));
    }

    #[test]
    fn implausible_percentage_rejects_snapshot() {
        let err = build_power_snapshot(power(true, Some(battery(101))), Ok((40, 80))).unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidData);
    }

    #[test]
    fn wire_types_never_carry_identity_strings() {
        let json = serde_json::to_string(
            &build_power_snapshot(power(true, Some(battery(80))), Ok((40, 80))).unwrap(),
        )
        .unwrap();
        assert!(!json.contains("DO-NOT-LEAK"));
        assert!(!json.contains("manufacturer"));
        assert!(!json.contains("serial"));
    }

    #[tokio::test]
    async fn gate_is_non_queuing() {
        let gate = std::sync::Arc::new(Hardware::new());
        let held = std::sync::Arc::clone(&gate);
        let first = tokio::task::spawn(async move {
            held.run_timed(Duration::from_secs(5), || {
                std::thread::sleep(Duration::from_millis(300))
            })
            .await
        });
        // Let the first operation engage the gate before competing.
        tokio::time::sleep(Duration::from_millis(100)).await;
        let second = gate.run_timed(Duration::from_secs(5), || ()).await;
        assert!(first.await.unwrap().is_ok());
        assert_eq!(second, Err(GateError::Busy));
    }

    #[tokio::test]
    async fn gate_timeout_leaves_gate_engaged() {
        let gate = Hardware::new();
        // 500ms sleep vs 50ms deadline: times out, worker still running.
        let first = gate
            .run_timed(Duration::from_millis(50), || {
                std::thread::sleep(Duration::from_millis(500))
            })
            .await;
        assert_eq!(first, Err(GateError::TimedOut));
        // A second call must not run beside the hung worker.
        let second = gate.run_timed(Duration::from_millis(50), || ()).await;
        assert_eq!(second, Err(GateError::Busy));
    }

    #[tokio::test]
    async fn gate_reports_worker_panic() {
        let gate = Hardware::new();
        let result = gate
            .run_timed(Duration::from_secs(5), || panic!("upstream did this"))
            .await;
        assert_eq!(result, Err(GateError::Panicked));
        // In production `run` aborts here; the gate must stay engaged either way.
        assert_eq!(
            gate.run_timed(Duration::from_millis(10), || ()).await,
            Err(GateError::Busy)
        );
    }

    #[test]
    fn features_advertise_power_reads_only() {
        let features = implemented_features();
        assert!(features.contains(&"battery".to_string()));
        assert!(features.contains(&"charge_limit_read".to_string()));
        assert!(!features.contains(&"charge_limit_write".to_string()));
    }
}
