// GameAnalytics Device Profiler - Frame Data Collector
// Captures basic per-frame timing: deltaTime, FPS, CPU/GPU time.

#if DEVELOPMENT_BUILD || UNITY_EDITOR

using UnityEngine;
#if UNITY_2022_1_OR_NEWER
using UnityEngine.Rendering;
#endif

namespace GameAnalytics.Profiler.Collectors
{
    public class FrameDataCollector : ICollector
    {
        private float _captureStartTime;

        public void OnCaptureStart()
        {
            _captureStartTime = Time.realtimeSinceStartup;
        }

        public void Collect(ref Data.FrameData frame)
        {
            frame.timestamp = Time.realtimeSinceStartup - _captureStartTime;
            frame.deltaTime = Time.unscaledDeltaTime;
            frame.fps = frame.deltaTime > 0f ? 1f / frame.deltaTime : 0f;

            // CPU/GPU timing via FrameTimingManager (Unity 2022.1+)
#if UNITY_2022_1_OR_NEWER
            FrameTimingManager.CaptureFrameTimings();
            var timings = new FrameTiming[1];
            uint count = FrameTimingManager.GetLatestTimings(1, timings);
            if (count > 0)
            {
                frame.cpuTimeMs = (float)timings[0].cpuMainThreadFrameTime;
                frame.gpuTimeMs = (float)timings[0].gpuFrameTime;
            }
            else
            {
                frame.cpuTimeMs = frame.deltaTime * 1000f;
                frame.gpuTimeMs = 0f;
            }
#else
            // Fallback: approximate CPU time from deltaTime, no GPU info
            frame.cpuTimeMs = frame.deltaTime * 1000f;
            frame.gpuTimeMs = 0f;
#endif
        }

        public void OnCaptureStop() { }
    }
}

#endif
