// GameAnalytics Profiler - iOS Native Temperature Plugin
// Exposes thermal state and CPU temperature to Unity via DllImport.

#import <Foundation/Foundation.h>

#ifdef __cplusplus
extern "C" {
#endif

/**
 * Returns the iOS thermal state as an integer:
 * 0 = Nominal, 1 = Fair, 2 = Serious, 3 = Critical
 * Maps to approximate temperature ranges:
 * Nominal: <35°C, Fair: 35-40°C, Serious: 40-45°C, Critical: >45°C
 */
int GAProfiler_GetThermalState() {
    NSProcessInfoThermalState state = [[NSProcessInfo processInfo] thermalState];
    return (int)state;
}

/**
 * Returns an estimated temperature in Celsius based on the thermal state.
 * This is an approximation since iOS does not expose exact temperature.
 */
float GAProfiler_GetEstimatedTemperature() {
    NSProcessInfoThermalState state = [[NSProcessInfo processInfo] thermalState];
    switch (state) {
        case NSProcessInfoThermalStateNominal:  return 30.0f;
        case NSProcessInfoThermalStateFair:     return 37.0f;
        case NSProcessInfoThermalStateSerious:  return 42.0f;
        case NSProcessInfoThermalStateCritical: return 48.0f;
        default: return -1.0f;
    }
}

/**
 * Returns the battery level (0.0 - 1.0).
 * Enables battery monitoring if not already enabled.
 */
float GAProfiler_GetBatteryLevel() {
    if (![[UIDevice currentDevice] isBatteryMonitoringEnabled]) {
        [[UIDevice currentDevice] setBatteryMonitoringEnabled:YES];
    }
    float level = [[UIDevice currentDevice] batteryLevel];
    return level; // -1.0 if unknown
}

#ifdef __cplusplus
}
#endif
