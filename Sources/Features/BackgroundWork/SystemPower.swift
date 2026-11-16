import Foundation
#if os(macOS)
import IOKit.ps
#endif

/// The Mac's real battery, charger and heat. Cheap: a few dictionary reads, called only when the system says something changed.
public struct SystemPower: PowerReading {
    public init() {}

    public func read() -> PowerConditions {
        let state = ProcessInfo.processInfo.thermalState
        let hot = state == .serious || state == .critical
        #if os(macOS)
        if let info = IOPSCopyPowerSourcesInfo()?.takeRetainedValue(),
           let sources = IOPSCopyPowerSourcesList(info)?.takeRetainedValue() as? [AnyObject] {
            for source in sources {
                guard let d = IOPSGetPowerSourceDescription(info, source)?.takeUnretainedValue() as? [String: Any],
                      (d[kIOPSTypeKey] as? String) == kIOPSInternalBatteryType else { continue }
                let current = d[kIOPSCurrentCapacityKey] as? Int ?? 100
                let full = d[kIOPSMaxCapacityKey] as? Int ?? 100
                let percent = full > 0 ? min(100, max(0, current * 100 / full)) : current
                let onMains = (d[kIOPSPowerSourceStateKey] as? String) == kIOPSACPowerValue
                return PowerConditions(batteryPercent: percent, charging: onMains, thermalSerious: hot)
            }
        }
        #endif
        // No internal battery (a desktop Mac, a rig without one): mains power, full.
        return PowerConditions(batteryPercent: 100, charging: true, thermalSerious: hot)
    }
}

/// `fixture power`: a scripted reading in place of the system's.
struct ScriptedPower: PowerReading {
    let conditions: PowerConditions
    func read() -> PowerConditions { conditions }
}
