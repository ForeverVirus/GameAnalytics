// GameAnalytics Device Profiler - Device Metrics Collector
// Battery level, temperature, device info.
// Temperature requires native plugins on Android/iOS.

#if DEVELOPMENT_BUILD || UNITY_EDITOR

using UnityEngine;
using System;
using System.Runtime.InteropServices;

namespace GameAnalytics.Profiler.Collectors
{
    public class DeviceMetricsCollector : ICollector
    {
        private float _lastBatteryPollTime;
        private float _cachedBattery;
        private float _cachedTemperature;
        private const float BatteryPollInterval = 2f; // Poll battery/temp every 2 seconds

#if UNITY_ANDROID && !UNITY_EDITOR
        private AndroidJavaObject _temperaturePlugin;
#endif

#if UNITY_IOS && !UNITY_EDITOR
        [DllImport("__Internal")]
        private static extern int _GAProfiler_GetThermalState();

        [DllImport("__Internal")]
        private static extern float _GAProfiler_GetBatteryLevel();
#endif

        public void OnCaptureStart()
        {
            _lastBatteryPollTime = -BatteryPollInterval; // Force immediate poll
            _cachedBattery = SystemInfo.batteryLevel;
            _cachedTemperature = 0f;

#if UNITY_ANDROID && !UNITY_EDITOR
            try
            {
                using (var pluginClass = new AndroidJavaClass("com.gameanalytics.profiler.TemperaturePlugin"))
                {
                    _temperaturePlugin = pluginClass.CallStatic<AndroidJavaObject>("getInstance");
                }
            }
            catch (Exception e)
            {
                Debug.LogWarning($"[GAProfiler] Android temperature plugin not available: {e.Message}");
            }
#endif
        }

        public void Collect(ref Data.FrameData frame)
        {
            float now = Time.realtimeSinceStartup;

            if (now - _lastBatteryPollTime >= BatteryPollInterval)
            {
                _lastBatteryPollTime = now;
                PollBatteryAndTemperature();
            }

            frame.batteryLevel = _cachedBattery;
            frame.temperature = _cachedTemperature;
        }

        private void PollBatteryAndTemperature()
        {
#if UNITY_ANDROID && !UNITY_EDITOR
            _cachedBattery = SystemInfo.batteryLevel;
            if (_temperaturePlugin != null)
            {
                try
                {
                    _cachedTemperature = _temperaturePlugin.Call<float>("getTemperature");
                }
                catch { }
            }
#elif UNITY_IOS && !UNITY_EDITOR
            _cachedBattery = _GAProfiler_GetBatteryLevel();
            // Map iOS thermal state (0-3) to approximate temperature
            int thermalState = _GAProfiler_GetThermalState();
            // 0=Nominal≈35, 1=Fair≈40, 2=Serious≈45, 3=Critical≈50
            _cachedTemperature = 35f + thermalState * 5f;
#else
            _cachedBattery = SystemInfo.batteryLevel;
            _cachedTemperature = 0f; // No temperature on Editor/PC
#endif
        }

        public void OnCaptureStop()
        {
#if UNITY_ANDROID && !UNITY_EDITOR
            _temperaturePlugin?.Dispose();
            _temperaturePlugin = null;
#endif
        }

        /// <summary>
        /// Captures one-time device information.
        /// </summary>
        public static Data.DeviceInfo CaptureDeviceInfo()
        {
            return new Data.DeviceInfo
            {
                deviceModel = SystemInfo.deviceModel,
                operatingSystem = SystemInfo.operatingSystem,
                processorType = SystemInfo.processorType,
                processorCount = SystemInfo.processorCount,
                processorFrequency = SystemInfo.processorFrequency,
                systemMemoryMB = SystemInfo.systemMemorySize,
                graphicsMemoryMB = SystemInfo.graphicsMemorySize,
                graphicsDeviceName = SystemInfo.graphicsDeviceName,
                graphicsDeviceType = SystemInfo.graphicsDeviceType.ToString(),
                screenWidth = Screen.width,
                screenHeight = Screen.height,
                screenRefreshRate = (int)Screen.currentResolution.refreshRateRatio.value,
                qualityLevel = QualitySettings.GetQualityLevel(),
                qualityName = QualitySettings.names.Length > 0
                    ? QualitySettings.names[QualitySettings.GetQualityLevel()]
                    : "Unknown",
                unityVersion = Application.unityVersion,
                sdkVersion = "1.0.0",
                projectName = Application.productName,
                buildGuid = Application.buildGUID
            };
        }
    }
}

#endif
