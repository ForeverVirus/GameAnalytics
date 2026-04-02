// GameAnalytics Profiler - Deep Profile Exporter
// Uses Unity Editor's official Profiler APIs to load .raw deep profile files
// and export them into a custom GADP binary format that the desktop app parses.

#if UNITY_EDITOR

using System;
using System.Collections.Generic;
using System.IO;
using System.Reflection;
using UnityEditor;
using UnityEditor.Profiling;
using UnityEditorInternal;
using UnityEngine;
using UProfiler = UnityEngine.Profiling.Profiler;

namespace GameAnalytics.Profiler.Editor
{
    public static class DeepProfileExporter
    {
        private static readonly byte[] Magic = System.Text.Encoding.ASCII.GetBytes("GADP");
        private const uint FormatVersion = 1;

        private struct ThreadMeta
        {
            public ulong threadId;
            public ushort threadIndex;
            public uint threadNameIndex;
            public uint groupNameIndex;
        }

        private sealed class FrameThreadExport
        {
            public ushort threadIndex;
            public readonly List<SampleExport> samples = new List<SampleExport>();
        }

        private sealed class FrameExport
        {
            public uint frameIndex;
            public ulong startTimeNs;
            public ulong durationNs;
            public readonly List<FrameThreadExport> threads = new List<FrameThreadExport>();
        }

        private struct SampleExport
        {
            public uint markerNameIndex;
            public ulong startTimeNs;
            public ulong durationNs;
            public byte depth;
            public ulong gcAllocBytes;
            public ushort category;
        }

        public static void ExportFromCommandLine()
        {
            string rawPath = GetArgValue("-gaDeepRawPath");
            string exportPath = GetArgValue("-gaDeepExportPath");

            if (string.IsNullOrEmpty(rawPath) || string.IsNullOrEmpty(exportPath))
            {
                Debug.LogError("[GAProfiler] Missing -gaDeepRawPath or -gaDeepExportPath");
                EditorApplication.Exit(2);
                return;
            }

            try
            {
                Export(rawPath, exportPath);
                Debug.Log($"[GAProfiler] Deep profile export complete: {exportPath}");
                EditorApplication.Exit(0);
            }
            catch (Exception ex)
            {
                Debug.LogError($"[GAProfiler] Deep profile export failed: {ex}");
                EditorApplication.Exit(1);
            }
        }

        public static string ExportForRuntime(string rawPath)
        {
            if (string.IsNullOrEmpty(rawPath))
                throw new ArgumentException("rawPath is null or empty", nameof(rawPath));

            string exportPath = rawPath.EndsWith(".raw", StringComparison.OrdinalIgnoreCase)
                ? rawPath.Substring(0, rawPath.Length - 4) + ".gadp"
                : rawPath + ".gadp";

            Export(rawPath, exportPath);
            return exportPath;
        }

        public static void Export(string rawPath, string exportPath)
        {
            if (!File.Exists(rawPath))
                throw new FileNotFoundException("Deep raw file not found", rawPath);

            Directory.CreateDirectory(Path.GetDirectoryName(exportPath) ?? ".");

            ClearLoadedProfilerFrames();
            UProfiler.AddFramesFromFile(rawPath);

            var profilerWindow = EditorWindow.GetWindow<ProfilerWindow>(false, "Profiler", false);

            int firstFrame = checked((int)profilerWindow.firstAvailableFrameIndex);
            int lastFrame = checked((int)profilerWindow.lastAvailableFrameIndex);
            if (firstFrame < 0 || lastFrame < firstFrame)
                throw new InvalidOperationException($"No profiler frames loaded from raw file: {rawPath}");

            var stringTable = new List<string>();
            var stringToIndex = new Dictionary<string, uint>(StringComparer.Ordinal);
            var markerNameIndexByMarkerId = new Dictionary<int, uint>();
            var threadMetaByKey = new Dictionary<string, ThreadMeta>(StringComparer.Ordinal);
            var frames = new List<FrameExport>();

            uint GetOrAddString(string value)
            {
                value ??= string.Empty;
                if (stringToIndex.TryGetValue(value, out uint idx))
                    return idx;
                idx = (uint)stringTable.Count;
                stringTable.Add(value);
                stringToIndex[value] = idx;
                return idx;
            }

            for (int frameIndex = firstFrame; frameIndex <= lastFrame; frameIndex++)
            {
                var frame = new FrameExport
                {
                    frameIndex = (uint)(frameIndex - firstFrame)
                };

                bool frameHasData = false;

                for (int threadIndex = 0; ; threadIndex++)
                {
                    using (var frameData = ProfilerDriver.GetRawFrameDataView(frameIndex, threadIndex))
                    {
                        if (!frameData.valid)
                            break;

                        if (!frameHasData)
                        {
                            frame.startTimeNs = frameData.frameStartTimeNs;
                            frame.durationNs = frameData.frameTimeNs;
                            frameHasData = true;
                        }

                        string threadName = string.IsNullOrEmpty(frameData.threadName)
                            ? $"Thread_{threadIndex}"
                            : frameData.threadName;
                        string groupName = string.IsNullOrEmpty(frameData.threadGroupName)
                            ? "Unknown"
                            : frameData.threadGroupName;

                        string threadKey = $"{frameData.threadId}:{frameData.threadIndex}:{threadName}:{groupName}";
                        if (!threadMetaByKey.ContainsKey(threadKey))
                        {
                            threadMetaByKey[threadKey] = new ThreadMeta
                            {
                                threadId = (ulong)frameData.threadId,
                                threadIndex = (ushort)frameData.threadIndex,
                                threadNameIndex = GetOrAddString(threadName),
                                groupNameIndex = GetOrAddString(groupName),
                            };
                        }

                        var thread = new FrameThreadExport
                        {
                            threadIndex = (ushort)frameData.threadIndex
                        };

                        var remainingChildren = new List<int>();
                        int sampleCount = frameData.sampleCount;

                        for (int sampleIndex = 0; sampleIndex < sampleCount; sampleIndex++)
                        {
                            while (remainingChildren.Count > 0 && remainingChildren[remainingChildren.Count - 1] == 0)
                                remainingChildren.RemoveAt(remainingChildren.Count - 1);

                            byte depth = (byte)remainingChildren.Count;
                            if (remainingChildren.Count > 0)
                                remainingChildren[remainingChildren.Count - 1]--;

                            int directChildren = frameData.GetSampleChildrenCount(sampleIndex);
                            remainingChildren.Add(directChildren);

                            int markerId = frameData.GetSampleMarkerId(sampleIndex);
                            if (!markerNameIndexByMarkerId.TryGetValue(markerId, out uint markerNameIndex))
                            {
                                markerNameIndex = GetOrAddString(frameData.GetSampleName(sampleIndex) ?? $"Marker_{markerId}");
                                markerNameIndexByMarkerId[markerId] = markerNameIndex;
                            }

                            thread.samples.Add(new SampleExport
                            {
                                markerNameIndex = markerNameIndex,
                                startTimeNs = frameData.GetSampleStartTimeNs(sampleIndex),
                                durationNs = frameData.GetSampleTimeNs(sampleIndex),
                                depth = depth,
                                gcAllocBytes = 0,
                                category = frameData.GetSampleCategoryIndex(sampleIndex),
                            });
                        }

                        frame.threads.Add(thread);
                    }
                }

                if (frameHasData)
                    frames.Add(frame);
            }

            WriteBinary(exportPath, stringTable, threadMetaByKey.Values, frames);
            profilerWindow.Close();
        }

        private static void ClearLoadedProfilerFrames()
        {
            var clearMethod = typeof(ProfilerDriver).GetMethod(
                "ClearAllFrames",
                BindingFlags.Static | BindingFlags.Public | BindingFlags.NonPublic);
            clearMethod?.Invoke(null, null);
        }

        private static void WriteBinary(
            string path,
            List<string> stringTable,
            ICollection<ThreadMeta> threads,
            List<FrameExport> frames)
        {
            using (var fs = new FileStream(path, FileMode.Create, FileAccess.Write, FileShare.None))
            using (var bw = new BinaryWriter(fs, System.Text.Encoding.UTF8))
            {
                bw.Write(Magic);
                bw.Write(FormatVersion);
                bw.Write((uint)stringTable.Count);
                bw.Write((uint)threads.Count);
                bw.Write((uint)frames.Count);

                foreach (var value in stringTable)
                {
                    byte[] bytes = System.Text.Encoding.UTF8.GetBytes(value ?? string.Empty);
                    bw.Write((uint)bytes.Length);
                    bw.Write(bytes);
                }

                foreach (var thread in threads)
                {
                    bw.Write(thread.threadId);
                    bw.Write(thread.threadIndex);
                    bw.Write(thread.threadNameIndex);
                    bw.Write(thread.groupNameIndex);
                }

                foreach (var frame in frames)
                {
                    bw.Write(frame.frameIndex);
                    bw.Write(frame.startTimeNs);
                    bw.Write(frame.durationNs);
                    bw.Write((ushort)frame.threads.Count);

                    foreach (var thread in frame.threads)
                    {
                        bw.Write(thread.threadIndex);
                        bw.Write((uint)thread.samples.Count);

                        foreach (var sample in thread.samples)
                        {
                            bw.Write(sample.markerNameIndex);
                            bw.Write(sample.startTimeNs);
                            bw.Write(sample.durationNs);
                            bw.Write(sample.depth);
                            bw.Write(sample.gcAllocBytes);
                            bw.Write(sample.category);
                        }
                    }
                }
            }
        }

        private static string GetArgValue(string key)
        {
            string[] args = Environment.GetCommandLineArgs();
            for (int i = 0; i < args.Length - 1; i++)
            {
                if (string.Equals(args[i], key, StringComparison.OrdinalIgnoreCase))
                    return args[i + 1];
            }
            return null;
        }
    }
}

#endif
