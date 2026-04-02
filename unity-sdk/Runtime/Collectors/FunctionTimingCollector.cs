// GameAnalytics Device Profiler - Function-Level Timing Collector
// Tracks 80+ Unity engine ProfilerRecorder markers to capture per-function
// CPU timing per frame, enabling UWA GOT-style thread call stack analysis.

#if DEVELOPMENT_BUILD || UNITY_EDITOR

using System;
using System.Collections.Generic;
using Unity.Profiling;
using GameAnalytics.Profiler.Data;

namespace GameAnalytics.Profiler.Collectors
{
    public class FunctionTimingCollector
    {
        private struct MarkerEntry
        {
            public string name;
            public string parentName;
            public FunctionCategory category;
            public ProfilerRecorder recorder;
        }

        private List<MarkerEntry> _markers = new List<MarkerEntry>();
        private CaptureSession _session;

        // Pre-defined engine markers matching UWA GOT's tracked functions
        private static readonly (string name, ProfilerCategory cat, FunctionCategory funcCat, string parentName)[] KnownMarkers = new[]
        {
            // Rendering
            ("Camera.Render", ProfilerCategory.Render, FunctionCategory.Rendering, null),
            ("RenderPipelineManager.DoRenderLoop_Internal", ProfilerCategory.Render, FunctionCategory.Rendering, null),
            ("PostLateUpdate.UpdateAllRenderers", ProfilerCategory.Render, FunctionCategory.Rendering, "Camera.Render"),
            ("Culling", ProfilerCategory.Render, FunctionCategory.Rendering, "Camera.Render"),
            ("Render.OpaqueGeometry", ProfilerCategory.Render, FunctionCategory.Rendering, "Camera.Render"),
            ("RenderForwardOpaque.Render", ProfilerCategory.Render, FunctionCategory.Rendering, "Render.OpaqueGeometry"),
            ("Render.TransparentGeometry", ProfilerCategory.Render, FunctionCategory.Rendering, "Camera.Render"),
            ("Shadows.RenderShadowMap", ProfilerCategory.Render, FunctionCategory.Rendering, "Camera.Render"),

            // GPU Sync / Wait
            ("Graphics.PresentAndSync", ProfilerCategory.Render, FunctionCategory.Sync, null),
            ("Gfx.WaitForPresentOnGfxThread", ProfilerCategory.Render, FunctionCategory.Sync, "Graphics.PresentAndSync"),
            ("TimeUpdate.WaitForLastPresentationAndUpdateTime", ProfilerCategory.Render, FunctionCategory.Sync, "Graphics.PresentAndSync"),
            ("EndGraphicsJobs", ProfilerCategory.Render, FunctionCategory.Sync, "Graphics.PresentAndSync"),

            // Scripts / User Code
            ("BehaviourUpdate", ProfilerCategory.Scripts, FunctionCategory.Scripting, null),
            ("LateBehaviourUpdate", ProfilerCategory.Scripts, FunctionCategory.Scripting, null),
            ("FixedBehaviourUpdate", ProfilerCategory.Scripts, FunctionCategory.Scripting, null),
            ("CoroutinesDelayedCalls", ProfilerCategory.Scripts, FunctionCategory.Scripting, "BehaviourUpdate"),
            ("Monobehaviour.OnMouse_", ProfilerCategory.Scripts, FunctionCategory.Scripting, "BehaviourUpdate"),
            ("PlayerEndOfFrame", ProfilerCategory.Scripts, FunctionCategory.Scripting, "LateBehaviourUpdate"),
            ("UnitySynchronizationContext.ExecuteTasks", ProfilerCategory.Scripts, FunctionCategory.Scripting, "BehaviourUpdate"),
            ("Application.InvokeOnBeforeRender", ProfilerCategory.Scripts, FunctionCategory.Scripting, "LateBehaviourUpdate"),
            ("NativeInputSystem.ShouldRunUpdate", ProfilerCategory.Scripts, FunctionCategory.Scripting, "BehaviourUpdate"),
            ("NativeInputSystem.NotifyBeforeUpdate", ProfilerCategory.Scripts, FunctionCategory.Scripting, "BehaviourUpdate"),
            ("NativeInputSystem.NotifyUpdate", ProfilerCategory.Scripts, FunctionCategory.Scripting, "BehaviourUpdate"),
            ("PlayerConnection.Poll", ProfilerCategory.Scripts, FunctionCategory.Scripting, "BehaviourUpdate"),
            ("AudioManager.Update", ProfilerCategory.Scripts, FunctionCategory.Scripting, "BehaviourUpdate"),
            ("TextureStreamingManager.Update", ProfilerCategory.Scripts, FunctionCategory.Scripting, "BehaviourUpdate"),
            ("EnlightenRuntimeManager.PostUpdate", ProfilerCategory.Scripts, FunctionCategory.Scripting, "LateBehaviourUpdate"),
            ("LightProbeProxyVolumeManager.Update", ProfilerCategory.Scripts, FunctionCategory.Scripting, "LateBehaviourUpdate"),
            ("ScriptableRuntimeReflectionSystemWrapper.Internal_ScriptableRuntimeReflectionSystemWrapper_TickRealtimeProbes", ProfilerCategory.Scripts, FunctionCategory.Scripting, "LateBehaviourUpdate"),
            ("OnDemandRendering.GetRenderFrameInterval", ProfilerCategory.Scripts, FunctionCategory.Scripting, "LateBehaviourUpdate"),
            ("SupportedRenderingFeatures.IsUIOverlayRenderedBySRP", ProfilerCategory.Scripts, FunctionCategory.Scripting, "LateBehaviourUpdate"),
            ("CustomRenderTextures.Update", ProfilerCategory.Scripts, FunctionCategory.Scripting, "LateBehaviourUpdate"),

            // Physics
            ("Physics.Simulate", ProfilerCategory.Physics, FunctionCategory.Physics, null),
            ("Physics.Processing", ProfilerCategory.Physics, FunctionCategory.Physics, "Physics.Simulate"),
            ("Physics.FetchResults", ProfilerCategory.Physics, FunctionCategory.Physics, "Physics.Simulate"),
            ("Physics.ProcessReports", ProfilerCategory.Physics, FunctionCategory.Physics, "Physics.Simulate"),
            ("Physics.Interpolation", ProfilerCategory.Physics, FunctionCategory.Physics, "Physics.Simulate"),
            ("Physics.UpdateBodies", ProfilerCategory.Physics, FunctionCategory.Physics, "Physics.Simulate"),
            ("Physics.UpdateCloth", ProfilerCategory.Physics, FunctionCategory.Physics, "Physics.Simulate"),
            ("Physics.UpdateVehicles", ProfilerCategory.Physics, FunctionCategory.Physics, "Physics.Simulate"),
            ("Physics2D.Simulate", ProfilerCategory.Physics, FunctionCategory.Physics, null),
            ("Physics2D.InterpolatePoses", ProfilerCategory.Physics, FunctionCategory.Physics, "Physics2D.Simulate"),

            // Animation
            ("DirectorUpdate", ProfilerCategory.Animation, FunctionCategory.Animation, null),
            ("Director.ProcessFrame", ProfilerCategory.Animation, FunctionCategory.Animation, "DirectorUpdate"),
            ("Director.PrepareFrame", ProfilerCategory.Animation, FunctionCategory.Animation, "DirectorUpdate"),
            ("Director.SampleTime", ProfilerCategory.Animation, FunctionCategory.Animation, "DirectorUpdate"),
            ("MeshSkinning.Update", ProfilerCategory.Animation, FunctionCategory.Animation, "DirectorUpdate"),

            // UI
            ("UI.LayoutUpdate", ProfilerCategory.Gui, FunctionCategory.UI, null),
            ("Rendering.UpdateBatches", ProfilerCategory.Gui, FunctionCategory.UI, "UI.LayoutUpdate"),
            ("UI.RenderOverlays", ProfilerCategory.Gui, FunctionCategory.UI, null),
            ("Rendering.RenderOverlays", ProfilerCategory.Gui, FunctionCategory.UI, "UI.RenderOverlays"),
            ("Rendering.EmitWorldScreenspaceCameraGeometry", ProfilerCategory.Gui, FunctionCategory.UI, "UI.RenderOverlays"),
            ("GUI.ProcessEvents", ProfilerCategory.Gui, FunctionCategory.UI, null),
            ("GUI.Repaint", ProfilerCategory.Gui, FunctionCategory.UI, "GUI.ProcessEvents"),

            // Loading
            ("Loading.PreloadManager", ProfilerCategory.Loading, FunctionCategory.Loading, null),
            ("Loading.AsyncRead", ProfilerCategory.Loading, FunctionCategory.Loading, "Loading.PreloadManager"),
            ("UpdatePreloading", ProfilerCategory.Loading, FunctionCategory.Loading, "Loading.PreloadManager"),

            // Particles
            ("ParticleSystem.Update", ProfilerCategory.Particles, FunctionCategory.Particles, null),
            ("ParticleSystem.EndUpdateAll", ProfilerCategory.Particles, FunctionCategory.Particles, "ParticleSystem.Update"),

            // GC
            ("GC.Collect", ProfilerCategory.Memory, FunctionCategory.GC, null),
        };

        public void Initialize(CaptureSession session)
        {
            _session = session;
        }

        public void OnCaptureStart()
        {
            _markers.Clear();
            foreach (var m in KnownMarkers)
            {
                var rec = ProfilerRecorder.StartNew(m.cat, m.name, 1);
                _markers.Add(new MarkerEntry
                {
                    name = m.name,
                    parentName = m.parentName,
                    category = m.funcCat,
                    recorder = rec
                });
            }
        }

        public List<FunctionSample> CollectSamples()
        {
            var samples = new List<FunctionSample>();
            var activeMarkers = new List<MarkerEntry>();
            var sampleIndexByName = new Dictionary<string, int>(StringComparer.Ordinal);

            for (int i = 0; i < _markers.Count; i++)
            {
                var m = _markers[i];
                if (!m.recorder.Valid) continue;

                long ns = m.recorder.LastValue;
                if (ns <= 0) continue;

                float ms = ns / 1_000_000f;
                ushort nameIdx = _session.GetOrAddString(m.name);

                sampleIndexByName[m.name] = samples.Count;
                activeMarkers.Add(m);
                samples.Add(new FunctionSample
                {
                    functionNameIndex = nameIdx,
                    category = m.category,
                    selfTimeMs = ms,
                    totalTimeMs = ms,
                    callCount = (ushort)System.Math.Max(1, m.recorder.Count),
                    depth = 0,
                    parentIndex = -1,
                    threadIndex = 0
                });
            }

            for (int i = 0; i < activeMarkers.Count; i++)
            {
                var marker = activeMarkers[i];
                if (string.IsNullOrEmpty(marker.parentName))
                    continue;

                if (sampleIndexByName.TryGetValue(marker.parentName, out int parentIndex))
                {
                    var sample = samples[i];
                    sample.parentIndex = (short)parentIndex;
                    sample.depth = (byte)(samples[parentIndex].depth + 1);
                    samples[i] = sample;
                }
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
