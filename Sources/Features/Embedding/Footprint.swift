import Foundation
#if canImport(Darwin)
import Darwin
#endif

/// The app's physical footprint, the figure the perf lane judges against RSS_MODEL_MAX_MB (task_vm_info.phys_footprint).
public enum Footprint {
    public static func physicalMB() -> Int? {
        #if canImport(Darwin)
        var info = task_vm_info_data_t()
        var count = mach_msg_type_number_t(MemoryLayout<task_vm_info_data_t>.size / MemoryLayout<natural_t>.size)
        let kr = withUnsafeMutablePointer(to: &info) {
            $0.withMemoryRebound(to: integer_t.self, capacity: Int(count)) { task_info(mach_task_self_, task_flavor_t(TASK_VM_INFO), $0, &count) }
        }
        return kr == KERN_SUCCESS ? Int(info.phys_footprint / 1_048_576) : nil
        #else
        return nil
        #endif
    }
}
