// GameAnalytics Device Profiler - Memory Collector
// Captures memory usage: total, reserved, mono heap, GPU, GC allocations.

#if DEVELOPMENT_BUILD || UNITY_EDITOR

using UnityEngine.Profiling;
using Unity.Profiling;

using UProfiler = UnityEngine.Profiling.Profiler;

namespace GameAnalytics.Profiler.Collectors
{
    public class MemoryCollector : ICollector
    {
        private ProfilerRecorder _gcAllocRecorder;

        public void OnCaptureStart()
        {
            _gcAllocRecorder = ProfilerRecorder.StartNew(ProfilerCategory.Memory, "GC Allocated In Frame");
        }

        public void Collect(ref Data.FrameData frame)
        {
            frame.totalAllocated = UProfiler.GetTotalAllocatedMemoryLong();
            frame.totalReserved = UProfiler.GetTotalReservedMemoryLong();
            frame.monoHeapSize = UProfiler.GetMonoHeapSizeLong();
            frame.monoUsedSize = UProfiler.GetMonoUsedSizeLong();
            frame.gfxMemory = UProfiler.GetAllocatedMemoryForGraphicsDriver();

            // Per-frame GC allocation in bytes
            frame.gcAllocBytes = _gcAllocRecorder.Valid ? _gcAllocRecorder.LastValue : 0;
        }

        public void OnCaptureStop()
        {
            _gcAllocRecorder.Dispose();
        }
    }
}

#endif
