// GameAnalytics Device Profiler - Rendering Stats Collector
// Captures batches, draw calls, set pass calls, triangles, vertices, etc.

#if DEVELOPMENT_BUILD || UNITY_EDITOR

using Unity.Profiling;

namespace GameAnalytics.Profiler.Collectors
{
    public class RenderingCollector : ICollector
    {
        private ProfilerRecorder _batches;
        private ProfilerRecorder _drawCalls;
        private ProfilerRecorder _setPassCalls;
        private ProfilerRecorder _triangles;
        private ProfilerRecorder _vertices;
        private ProfilerRecorder _shadowCasters;
        private ProfilerRecorder _skinnedMeshes;

        public void OnCaptureStart()
        {
            _batches = ProfilerRecorder.StartNew(ProfilerCategory.Render, "Batches Count");
            _drawCalls = ProfilerRecorder.StartNew(ProfilerCategory.Render, "Draw Calls Count");
            _setPassCalls = ProfilerRecorder.StartNew(ProfilerCategory.Render, "SetPass Calls Count");
            _triangles = ProfilerRecorder.StartNew(ProfilerCategory.Render, "Triangles Count");
            _vertices = ProfilerRecorder.StartNew(ProfilerCategory.Render, "Vertices Count");
            _shadowCasters = ProfilerRecorder.StartNew(ProfilerCategory.Render, "Shadow Casters Count");
            _skinnedMeshes = ProfilerRecorder.StartNew(ProfilerCategory.Render, "Visible Skinned Meshes Count");
        }

        public void Collect(ref Data.FrameData frame)
        {
            frame.batches = (int)GetValue(_batches);
            frame.drawCalls = (int)GetValue(_drawCalls);
            frame.setPassCalls = (int)GetValue(_setPassCalls);
            frame.triangles = (int)GetValue(_triangles);
            frame.vertices = (int)GetValue(_vertices);
            frame.shadowCasters = (int)GetValue(_shadowCasters);
            frame.visibleSkinnedMeshes = (int)GetValue(_skinnedMeshes);
        }

        public void OnCaptureStop()
        {
            _batches.Dispose();
            _drawCalls.Dispose();
            _setPassCalls.Dispose();
            _triangles.Dispose();
            _vertices.Dispose();
            _shadowCasters.Dispose();
            _skinnedMeshes.Dispose();
        }

        private static long GetValue(ProfilerRecorder rec)
        {
            return rec.Valid && rec.LastValue >= 0 ? rec.LastValue : 0;
        }
    }
}

#endif
