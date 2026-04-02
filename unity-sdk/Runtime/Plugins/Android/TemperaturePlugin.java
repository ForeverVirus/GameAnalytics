package com.gameanalytics.profiler;

import android.content.Context;
import android.content.Intent;
import android.content.IntentFilter;
import android.os.BatteryManager;

/**
 * Native Android plugin to read device temperature from BatteryManager.
 * Called via AndroidJavaObject from Unity C# code.
 */
public class TemperaturePlugin {

    /**
     * Returns the battery temperature in degrees Celsius.
     * Uses BatteryManager.EXTRA_TEMPERATURE which returns tenths of a degree.
     */
    public static float getBatteryTemperature(Context context) {
        try {
            IntentFilter filter = new IntentFilter(Intent.ACTION_BATTERY_CHANGED);
            Intent batteryStatus = context.registerReceiver(null, filter);
            if (batteryStatus != null) {
                int temp = batteryStatus.getIntExtra(BatteryManager.EXTRA_TEMPERATURE, 0);
                return temp / 10.0f;
            }
        } catch (Exception e) {
            // Return -1 on failure
        }
        return -1f;
    }

    /**
     * Returns the battery level as a float from 0.0 to 1.0.
     */
    public static float getBatteryLevel(Context context) {
        try {
            IntentFilter filter = new IntentFilter(Intent.ACTION_BATTERY_CHANGED);
            Intent batteryStatus = context.registerReceiver(null, filter);
            if (batteryStatus != null) {
                int level = batteryStatus.getIntExtra(BatteryManager.EXTRA_LEVEL, -1);
                int scale = batteryStatus.getIntExtra(BatteryManager.EXTRA_SCALE, -1);
                if (level >= 0 && scale > 0) {
                    return level / (float) scale;
                }
            }
        } catch (Exception e) {
            // Return -1 on failure
        }
        return -1f;
    }
}
