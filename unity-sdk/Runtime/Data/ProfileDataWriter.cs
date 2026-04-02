// GameAnalytics Device Profiler - Binary .gaprof Writer
// Custom binary format for efficient storage and fast parsing.

#if DEVELOPMENT_BUILD || UNITY_EDITOR

using System;
using System.IO;
using System.Text;
using UnityEngine;

namespace GameAnalytics.Profiler.Data
{
    /// <summary>
    /// Writes a CaptureSession to the .gaprof binary format.
    ///
    /// Format Layout:
    /// [Header 64 bytes]
    /// [DeviceInfo Block - UTF8 JSON]
    /// [StringTable Block]
    /// [FrameData Block - fixed-size per frame]
    /// [Screenshot Index + Data Block]
    /// [Overdraw Block]
    /// </summary>
    public class ProfileDataWriter
    {
        // Magic bytes
        private static readonly byte[] Magic = Encoding.ASCII.GetBytes("GAPROF");
        private const ushort FormatVersion = 3;

        // Module flag bits
        [Flags]
        public enum ModuleFlags : uint
        {
            FrameData    = 1 << 0,
            Memory       = 1 << 1,
            Rendering    = 1 << 2,
            ModuleTiming = 1 << 3,
            Jank         = 1 << 4,
            DeviceMetrics= 1 << 5,
            Screenshots  = 1 << 6,
            Overdraw     = 1 << 7,
            FunctionSamples = 1 << 8,
            LogEntries   = 1 << 9,
            ResourceMemory = 1 << 10,
            GPUAnalysis  = 1 << 11,
            CustomModules= 1 << 12,
        }

        // Per-frame binary size for v3:
        // 155-byte base frame + 9 x i64 resource memory (72) + 2 x f32 GPU metrics (8) = 235 bytes
        private const int FrameByteSize = 235;

        public void Write(CaptureSession session, string filePath, Action<float> onProgress = null)
        {
            using (var fs = new FileStream(filePath, FileMode.Create, FileAccess.Write, FileShare.None, 65536))
            using (var bw = new BinaryWriter(fs, Encoding.UTF8))
            {
                // ---- Prepare blocks ----
                string deviceInfoJson = JsonUtility.ToJson(session.deviceInfo);
                byte[] deviceInfoBytes = Encoding.UTF8.GetBytes(deviceInfoJson);

                // String table: [count:u16] [length:u16 + UTF8 bytes] ...
                byte[] stringTableBytes;
                using (var ms = new MemoryStream())
                using (var sw = new BinaryWriter(ms))
                {
                    sw.Write((ushort)session.stringTable.Count);
                    foreach (var s in session.stringTable)
                    {
                        byte[] sb = Encoding.UTF8.GetBytes(s);
                        sw.Write((ushort)sb.Length);
                        sw.Write(sb);
                    }
                    stringTableBytes = ms.ToArray();
                }

                // Calculate offsets
                long headerSize = 64;
                long deviceInfoOffset = headerSize;
                long stringTableOffset = deviceInfoOffset + 4 + deviceInfoBytes.Length; // 4 for length prefix
                long frameDataOffset = stringTableOffset + stringTableBytes.Length;
                long screenshotIndexOffset = frameDataOffset + (long)session.frames.Count * FrameByteSize;

                // Screenshot block size
                long screenshotBlockSize = 0;
                // Index: count(u16) + per entry (frameIndex:u32 + offset:u64 + size:u32) = 2 + N*16
                screenshotBlockSize += 2 + session.screenshots.Count * 16;
                long screenshotDataStart = screenshotIndexOffset + screenshotBlockSize;
                // Actual JPEG data
                long[] ssOffsets = new long[session.screenshots.Count];
                long runningOffset = screenshotDataStart;
                for (int i = 0; i < session.screenshots.Count; i++)
                {
                    ssOffsets[i] = runningOffset;
                    runningOffset += session.screenshots[i].jpegData.Length;
                }
                long overdrawOffset = runningOffset;

                // ---- Write Header (64 bytes) ----
                bw.Write(Magic);                                        // 6
                bw.Write(FormatVersion);                                // 2
                bw.Write((uint)GetFlags(session));                     // 4
                bw.Write((uint)session.frames.Count);                  // 4
                bw.Write((double)session.duration);                    // 8
                bw.Write((ushort)session.screenshots.Count);           // 2
                bw.Write((ulong)deviceInfoOffset);                     // 8
                bw.Write((ulong)frameDataOffset);                      // 8
                bw.Write((ulong)screenshotIndexOffset);                // 8
                bw.Write((ulong)overdrawOffset);                       // 8
                // Pad to 64 bytes
                int headerWritten = 6 + 2 + 4 + 4 + 8 + 2 + 8 + 8 + 8 + 8;
                int pad = 64 - headerWritten;
                if (pad > 0) bw.Write(new byte[pad]);

                // ---- Write DeviceInfo Block ----
                bw.Write((int)deviceInfoBytes.Length);
                bw.Write(deviceInfoBytes);

                // ---- Write StringTable Block ----
                bw.Write(stringTableBytes);

                // ---- Write FrameData Block ----
                int total = session.frames.Count;
                for (int i = 0; i < total; i++)
                {
                    WriteFrame(bw, session.frames[i]);
                    if (i % 1000 == 0)
                        onProgress?.Invoke((float)i / total * 0.8f);
                }

                // ---- Write Screenshot Index ----
                bw.Write((ushort)session.screenshots.Count);
                for (int i = 0; i < session.screenshots.Count; i++)
                {
                    bw.Write((uint)session.screenshots[i].frameIndex);
                    bw.Write((ulong)ssOffsets[i]);
                    bw.Write((uint)session.screenshots[i].jpegData.Length);
                }

                // ---- Write Screenshot Data ----
                for (int i = 0; i < session.screenshots.Count; i++)
                {
                    bw.Write(session.screenshots[i].jpegData);
                }
                onProgress?.Invoke(0.9f);

                // ---- Write Overdraw Block ----
                bw.Write((ushort)session.overdrawSamples.Count);
                foreach (var od in session.overdrawSamples)
                {
                    bw.Write((uint)od.frameIndex);
                    bw.Write(od.timestamp);
                    bw.Write(od.avgOverdrawLayers);
                    bw.Write((uint)(od.heatmapJpeg?.Length ?? 0));
                    if (od.heatmapJpeg != null && od.heatmapJpeg.Length > 0)
                        bw.Write(od.heatmapJpeg);
                }

                // ---- Write FunctionSamples Block (v2) ----
                if (session.deepProfilingEnabled && session.frameFunctionSamples.Count > 0)
                {
                    bw.Write((uint)session.frameFunctionSamples.Count);
                    for (int i = 0; i < session.frameFunctionSamples.Count; i++)
                    {
                        var samples = session.frameFunctionSamples[i];
                        if (samples == null || samples.Count == 0)
                        {
                            bw.Write((ushort)0);
                            continue;
                        }
                        bw.Write((ushort)samples.Count);
                        foreach (var s in samples)
                        {
                            bw.Write(s.functionNameIndex);   // u16
                            bw.Write((byte)s.category);      // u8
                            bw.Write(s.selfTimeMs);           // f32
                            bw.Write(s.totalTimeMs);          // f32
                            bw.Write(s.callCount);            // u16
                            bw.Write(s.depth);                // u8
                            bw.Write(s.parentIndex);          // i16
                            bw.Write(s.threadIndex);          // u8 (v3)
                        }
                    }
                }

                // ---- Write LogEntries Block (v2) ----
                if (session.logEntries.Count > 0)
                {
                    bw.Write((uint)session.logEntries.Count);
                    foreach (var log in session.logEntries)
                    {
                        bw.Write(log.timestamp);
                        bw.Write(log.frameIndex);
                        bw.Write((byte)log.logType);
                        byte[] msgBytes = Encoding.UTF8.GetBytes(log.message ?? "");
                        bw.Write((ushort)msgBytes.Length);
                        bw.Write(msgBytes);
                        byte[] stBytes = Encoding.UTF8.GetBytes(log.stackTrace ?? "");
                        bw.Write((ushort)stBytes.Length);
                        bw.Write(stBytes);
                    }
                }

                // ---- Write ResourceMemory Block (v3) ----
                if (session.resourceSnapshots.Count > 0)
                {
                    WriteResourceMemoryBlock(bw, session);
                }

                onProgress?.Invoke(1f);
            }
        }

        private void WriteFrame(BinaryWriter bw, FrameData f)
        {
            // Timing (5 × f32 = 20 bytes)
            bw.Write(f.timestamp);
            bw.Write(f.deltaTime);
            bw.Write(f.fps);
            bw.Write(f.cpuTimeMs);
            bw.Write(f.gpuTimeMs);

            // Module timings (12 × f32 = 48 bytes)
            bw.Write(f.renderTime);
            bw.Write(f.scriptsUpdateTime);
            bw.Write(f.scriptsLateUpdateTime);
            bw.Write(f.physicsTime);
            bw.Write(f.animationTime);
            bw.Write(f.uiTime);
            bw.Write(f.particleTime);
            bw.Write(f.loadingTime);
            bw.Write(f.gcCollectTime);
            bw.Write(f.fixedUpdateTime);
            bw.Write(f.renderSubmitTime);
            bw.Write(f.otherTime);

            // Memory (6 × i64 = 48 bytes)
            bw.Write(f.totalAllocated);
            bw.Write(f.totalReserved);
            bw.Write(f.monoHeapSize);
            bw.Write(f.monoUsedSize);
            bw.Write(f.gfxMemory);
            bw.Write(f.gcAllocBytes);

            // Rendering (7 × i32 = 28 bytes)
            bw.Write(f.batches);
            bw.Write(f.drawCalls);
            bw.Write(f.setPassCalls);
            bw.Write(f.triangles);
            bw.Write(f.vertices);
            bw.Write(f.shadowCasters);
            bw.Write(f.visibleSkinnedMeshes);

            // Jank (1 byte)
            bw.Write(f.jankLevel);

            // Hardware (2 × f32 = 8 bytes)
            bw.Write(f.batteryLevel);
            bw.Write(f.temperature);

            // Scene index (u16 = 2 bytes)
            bw.Write(f.sceneIndex);

            // v3 fields: 9 × i64 (72 bytes) + 2 × f32 (8 bytes) = 80 bytes
            bw.Write(f.textureMemory);
            bw.Write(f.meshMemory);
            bw.Write(f.materialMemory);
            bw.Write(f.shaderMemory);
            bw.Write(f.animClipMemory);
            bw.Write(f.audioClipMemory);
            bw.Write(f.fontMemory);
            bw.Write(f.renderTextureMemory);
            bw.Write(f.particleSystemMemory);
            bw.Write(f.gpuUtilization);
            bw.Write(f.cpuFrequencyMhz);
        }

        private uint GetFlags(CaptureSession session)
        {
            uint flags = (uint)ModuleFlags.FrameData;
            // Infer from data presence
            if (session.frames.Count > 0)
            {
                var f = session.frames[0];
                if (f.totalAllocated > 0) flags |= (uint)ModuleFlags.Memory;
                if (f.batches > 0 || f.drawCalls > 0) flags |= (uint)ModuleFlags.Rendering;
                if (f.renderTime > 0) flags |= (uint)ModuleFlags.ModuleTiming;
                if (f.jankLevel > 0) flags |= (uint)ModuleFlags.Jank;
                else flags |= (uint)ModuleFlags.Jank; // Always include jank flag
                if (f.batteryLevel > 0 || f.temperature > 0) flags |= (uint)ModuleFlags.DeviceMetrics;
            }
            if (session.screenshots.Count > 0) flags |= (uint)ModuleFlags.Screenshots;
            if (session.overdrawSamples.Count > 0) flags |= (uint)ModuleFlags.Overdraw;
            if (session.deepProfilingEnabled && session.frameFunctionSamples.Count > 0) flags |= (uint)ModuleFlags.FunctionSamples;
            if (session.logEntries.Count > 0) flags |= (uint)ModuleFlags.LogEntries;
            if (session.resourceSnapshots.Count > 0) flags |= (uint)ModuleFlags.ResourceMemory;
            if (session.frames.Count > 0 && session.frames[0].gpuUtilization > 0) flags |= (uint)ModuleFlags.GPUAnalysis;
            if (session.customMarkerNames.Count > 0) flags |= (uint)ModuleFlags.CustomModules;
            return flags;
        }

        /// <summary>
        /// Writes the resource memory detail block after log entries.
        /// Called from Write() after the main blocks.
        /// </summary>
        private void WriteResourceMemoryBlock(BinaryWriter bw, CaptureSession session)
        {
            var snapshots = session.resourceSnapshots;
            bw.Write((uint)snapshots.Count);
            foreach (var snap in snapshots)
            {
                bw.Write((uint)snap.frameIndex);
                bw.Write(snap.totalMemory);

                WriteResourceList(bw, snap.textures, session);
                WriteResourceList(bw, snap.meshes, session);
                WriteResourceList(bw, snap.materials, session);
                WriteResourceList(bw, snap.shaders, session);
                WriteResourceList(bw, snap.animClips, session);
                WriteResourceList(bw, snap.audioClips, session);
                WriteResourceList(bw, snap.fonts, session);
                WriteResourceList(bw, snap.renderTextures, session);
                WriteResourceList(bw, snap.particleSystems, session);
            }
        }

        private void WriteResourceList(BinaryWriter bw, System.Collections.Generic.List<Collectors.ResourceInstanceInfo> instances, CaptureSession session)
        {
            bw.Write((ushort)instances.Count);
            foreach (var inst in instances)
            {
                bw.Write(session.GetOrAddString(inst.name ?? "<unnamed>"));
                bw.Write(inst.sizeBytes);
            }
        }
    }
}

#endif
