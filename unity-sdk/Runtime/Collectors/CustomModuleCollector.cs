// GameAnalytics Device Profiler - Custom Module Collector
// Tracks user-defined Profiler markers for custom module analysis.
// Marker names are configured in GAProfilerConfig.customMarkerNames.

#if DEVELOPMENT_BUILD || UNITY_EDITOR

using System.Collections.Generic;
using Unity.Profiling;
using GameAnalytics.Profiler.Data;

namespace GameAnalytics.Profiler.Collectors
{
    /// <summary>
    /// Reads user-defined marker names from <see cref="GAProfilerConfig.customMarkerNames"/>
    /// and creates ProfilerRecorders to capture their timing.
    /// Results are added as FunctionSample entries with category = Custom (11).
    /// </summary>
    public class CustomModuleCollector
    {
        private struct CustomMarker
        {
            public string name;
            public ProfilerRecorder recorder;
        }

        private List<CustomMarker> _markers = new List<CustomMarker>();
        private CaptureSession _session;
        private string[] _markerNames;

        public CustomModuleCollector(string[] markerNames)
        {
            _markerNames = markerNames ?? new string[0];
        }

        public void Initialize(CaptureSession session)
        {
            _session = session;
        }

        public void OnCaptureStart()
        {
            _markers.Clear();
            foreach (var name in _markerNames)
            {
                if (string.IsNullOrWhiteSpace(name)) continue;
                var trimmed = name.Trim();
                var rec = ProfilerRecorder.StartNew(ProfilerCategory.Scripts, trimmed, 1);
                _markers.Add(new CustomMarker
                {
                    name = trimmed,
                    recorder = rec,
                });
            }
        }

        /// <summary>
        /// Collect timing samples for all custom markers.
        /// Returns a list of FunctionSample to be appended to the frame's function samples.
        /// </summary>
        public List<FunctionSample> CollectSamples()
        {
            var samples = new List<FunctionSample>();

            for (int i = 0; i < _markers.Count; i++)
            {
                var m = _markers[i];
                if (!m.recorder.Valid) continue;

                long ns = m.recorder.LastValue;
                if (ns <= 0) continue;

                float ms = ns / 1_000_000f;
                ushort nameIdx = _session.GetOrAddString(m.name);

                samples.Add(new FunctionSample
                {
                    functionNameIndex = nameIdx,
                    category = (FunctionCategory)11,  // Custom
                    selfTimeMs = ms,
                    totalTimeMs = ms,
                    callCount = 1,
                    depth = 0,
                    parentIndex = -1,
                });
            }

            return samples;
        }

        public void OnCaptureStop()
        {
            foreach (var m in _markers)
            {
                if (m.recorder.Valid)
                    m.recorder.Dispose();
            }
            _markers.Clear();
        }
    }
}

#endif
