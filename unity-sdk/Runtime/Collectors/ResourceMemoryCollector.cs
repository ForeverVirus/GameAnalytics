// GameAnalytics Device Profiler - Resource Memory Collector
// Captures per-resource-type memory usage via Profiler.GetRuntimeMemorySizeLong.
// Samples resource instances every N frames to avoid performance overhead.

#if DEVELOPMENT_BUILD || UNITY_EDITOR

using System;
using System.Collections.Generic;
using System.Linq;
using UnityEngine;
using UnityEngine.Profiling;
using GameAnalytics.Profiler.Data;

using UProfiler = UnityEngine.Profiling.Profiler;

namespace GameAnalytics.Profiler.Collectors
{
    /// <summary>
    /// Tracks per-resource-type runtime memory (Texture, Mesh, Material, Shader,
    /// AnimationClip, AudioClip, Font, RenderTexture, ParticleSystem).
    /// Writes aggregate totals every frame; detailed top-N instance lists every
    /// <see cref="sampleInterval"/> frames.
    /// </summary>
    public class ResourceMemoryCollector : ICollector
    {
        private int _sampleInterval;
        private int _frameCounter;
        private CaptureSession _session;

        // Per-type cached counts (bytes) — updated every sample interval
        private long _textureMemory;
        private long _meshMemory;
        private long _materialMemory;
        private long _shaderMemory;
        private long _animClipMemory;
        private long _audioClipMemory;
        private long _fontMemory;
        private long _renderTextureMemory;
        private long _particleSystemMemory;

        /// <summary>
        /// Detailed per-type / per-instance breakdown captured every sample interval.
        /// The Rust backend reads this from a separate block in the .gaprof file.
        /// </summary>
        public List<ResourceMemorySnapshot> Snapshots { get; } = new List<ResourceMemorySnapshot>();

        public ResourceMemoryCollector(int sampleInterval = 30)
        {
            _sampleInterval = Mathf.Max(1, sampleInterval);
        }

        public void Initialize(CaptureSession session)
        {
            _session = session;
        }

        public void OnCaptureStart()
        {
            _frameCounter = 0;
            Snapshots.Clear();
            // Take an initial sample immediately
            SampleResources(0);
        }

        public void Collect(ref FrameData frame)
        {
            _frameCounter++;

            if (_frameCounter % _sampleInterval == 0)
            {
                SampleResources(_session?.frames.Count ?? _frameCounter);
            }

            // Write cached aggregate values to frame
            frame.textureMemory = _textureMemory;
            frame.meshMemory = _meshMemory;
            frame.materialMemory = _materialMemory;
            frame.shaderMemory = _shaderMemory;
            frame.animClipMemory = _animClipMemory;
            frame.audioClipMemory = _audioClipMemory;
            frame.fontMemory = _fontMemory;
            frame.renderTextureMemory = _renderTextureMemory;
            frame.particleSystemMemory = _particleSystemMemory;
        }

        public void OnCaptureStop()
        {
            // Final sample
            SampleResources(_session?.frames.Count ?? _frameCounter);
        }

        private void SampleResources(int frameIndex)
        {
            var snapshot = new ResourceMemorySnapshot { frameIndex = frameIndex };

            _textureMemory = SampleType<Texture>(snapshot.textures);
            _meshMemory = SampleType<Mesh>(snapshot.meshes);
            _materialMemory = SampleType<Material>(snapshot.materials);
            _shaderMemory = SampleType<Shader>(snapshot.shaders);
            _animClipMemory = SampleType<AnimationClip>(snapshot.animClips);
            _audioClipMemory = SampleType<AudioClip>(snapshot.audioClips);
            _fontMemory = SampleType<Font>(snapshot.fonts);
            _renderTextureMemory = SampleType<RenderTexture>(snapshot.renderTextures);
            _particleSystemMemory = SampleType<ParticleSystem>(snapshot.particleSystems);

            snapshot.totalMemory = _textureMemory + _meshMemory + _materialMemory
                + _shaderMemory + _animClipMemory + _audioClipMemory + _fontMemory
                + _renderTextureMemory + _particleSystemMemory;

            Snapshots.Add(snapshot);
        }

        private static long SampleType<T>(List<ResourceInstanceInfo> outInstances) where T : UnityEngine.Object
        {
            long total = 0;
            var objects = Resources.FindObjectsOfTypeAll<T>();

            foreach (var obj in objects)
            {
                long size = UProfiler.GetRuntimeMemorySizeLong(obj);
                total += size;
                outInstances.Add(new ResourceInstanceInfo
                {
                    name = obj.name,
                    sizeBytes = size,
                });
            }

            // Sort descending by size, keep top 50 per type
            outInstances.Sort((a, b) => b.sizeBytes.CompareTo(a.sizeBytes));
            if (outInstances.Count > 50)
                outInstances.RemoveRange(50, outInstances.Count - 50);

            return total;
        }
    }

    /// <summary>
    /// Snapshot of resource memory at a specific frame — detailed per-type breakdown.
    /// </summary>
    [Serializable]
    public class ResourceMemorySnapshot
    {
        public int frameIndex;
        public long totalMemory;

        public List<ResourceInstanceInfo> textures = new List<ResourceInstanceInfo>();
        public List<ResourceInstanceInfo> meshes = new List<ResourceInstanceInfo>();
        public List<ResourceInstanceInfo> materials = new List<ResourceInstanceInfo>();
        public List<ResourceInstanceInfo> shaders = new List<ResourceInstanceInfo>();
        public List<ResourceInstanceInfo> animClips = new List<ResourceInstanceInfo>();
        public List<ResourceInstanceInfo> audioClips = new List<ResourceInstanceInfo>();
        public List<ResourceInstanceInfo> fonts = new List<ResourceInstanceInfo>();
        public List<ResourceInstanceInfo> renderTextures = new List<ResourceInstanceInfo>();
        public List<ResourceInstanceInfo> particleSystems = new List<ResourceInstanceInfo>();
    }

    /// <summary>
    /// A single resource instance with name and runtime memory size.
    /// </summary>
    [Serializable]
    public struct ResourceInstanceInfo
    {
        public string name;
        public long sizeBytes;
    }
}

#endif
