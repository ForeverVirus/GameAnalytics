// GameAnalytics Profiler - Editor Integration
// Provides menu items and custom inspector for GAProfilerConfig.

using UnityEditor;
using UnityEngine;

namespace GameAnalytics.Profiler.Editor
{
    public static class GAProfilerMenu
    {
        [MenuItem("GameAnalytics/Profiler/Create Config Asset", false, 100)]
        private static void CreateConfigAsset()
        {
            var config = ScriptableObject.CreateInstance<GAProfilerConfig>();
            string path = EditorUtility.SaveFilePanelInProject(
                "Save GAProfiler Config", "GAProfilerConfig", "asset",
                "Choose where to save the profiler config asset.");
            if (!string.IsNullOrEmpty(path))
            {
                AssetDatabase.CreateAsset(config, path);
                AssetDatabase.SaveAssets();
                Selection.activeObject = config;
                EditorUtility.FocusProjectWindow();
            }
        }

        [MenuItem("GameAnalytics/Profiler/Add Profiler to Scene", false, 101)]
        private static void AddProfilerToScene()
        {
            if (Object.FindObjectOfType<GAProfiler>() != null)
            {
                EditorUtility.DisplayDialog("GAProfiler",
                    "A GAProfiler instance already exists in the scene.", "OK");
                return;
            }

            var go = new GameObject("GAProfiler");
            go.AddComponent<GAProfiler>();
            go.AddComponent<UI.ProfilerOverlay>();
            Selection.activeGameObject = go;
            Undo.RegisterCreatedObjectUndo(go, "Add GAProfiler");
        }

        [MenuItem("GameAnalytics/Profiler/Open Data Folder", false, 200)]
        private static void OpenDataFolder()
        {
            string path = System.IO.Path.Combine(Application.persistentDataPath, "GameAnalytics");
            if (!System.IO.Directory.Exists(path))
                System.IO.Directory.CreateDirectory(path);
            EditorUtility.RevealInFinder(path);
        }
    }

    [CustomEditor(typeof(GAProfilerConfig))]
    public class GAProfilerConfigEditor : UnityEditor.Editor
    {
        private SerializedProperty _targetFps;
        private SerializedProperty _sampleEveryNFrames;
        private SerializedProperty _enableMemory, _enableRendering, _enableModuleTiming;
        private SerializedProperty _enableJankDetection, _enableDeviceMetrics;
        private SerializedProperty _enableScreenshots, _enableOverdraw;
        private SerializedProperty _screenshotInterval, _screenshotThumbnailHeight, _screenshotJpegQuality;
        private SerializedProperty _fpsDropScreenshotThreshold;
        private SerializedProperty _overdrawSampleInterval, _overdrawShader;
        private SerializedProperty _enableWifiTransfer, _httpServerPort;
        private SerializedProperty _autoStartCapture, _autoStartSessionName;
        private SerializedProperty _enableDeepProfiling, _captureLogs, _deepProfilingSampleRate;
        private SerializedProperty _enableResourceMemory, _resourceSampleInterval;
        private SerializedProperty _enableGPUAnalysis;
        private SerializedProperty _customMarkerNames;

        private void OnEnable()
        {
            _targetFps = serializedObject.FindProperty("targetFps");
            _sampleEveryNFrames = serializedObject.FindProperty("sampleEveryNFrames");
            _enableMemory = serializedObject.FindProperty("enableMemory");
            _enableRendering = serializedObject.FindProperty("enableRendering");
            _enableModuleTiming = serializedObject.FindProperty("enableModuleTiming");
            _enableJankDetection = serializedObject.FindProperty("enableJankDetection");
            _enableDeviceMetrics = serializedObject.FindProperty("enableDeviceMetrics");
            _enableScreenshots = serializedObject.FindProperty("enableScreenshots");
            _enableOverdraw = serializedObject.FindProperty("enableOverdraw");
            _screenshotInterval = serializedObject.FindProperty("screenshotInterval");
            _screenshotThumbnailHeight = serializedObject.FindProperty("screenshotThumbnailHeight");
            _screenshotJpegQuality = serializedObject.FindProperty("screenshotJpegQuality");
            _fpsDropScreenshotThreshold = serializedObject.FindProperty("fpsDropScreenshotThreshold");
            _overdrawSampleInterval = serializedObject.FindProperty("overdrawSampleInterval");
            _overdrawShader = serializedObject.FindProperty("overdrawShader");
            _enableWifiTransfer = serializedObject.FindProperty("enableWifiTransfer");
            _httpServerPort = serializedObject.FindProperty("httpServerPort");
            _autoStartCapture = serializedObject.FindProperty("autoStartCapture");
            _autoStartSessionName = serializedObject.FindProperty("autoStartSessionName");
            _enableDeepProfiling = serializedObject.FindProperty("enableDeepProfiling");
            _captureLogs = serializedObject.FindProperty("captureLogs");
            _deepProfilingSampleRate = serializedObject.FindProperty("deepProfilingSampleRate");
            _enableResourceMemory = serializedObject.FindProperty("enableResourceMemory");
            _resourceSampleInterval = serializedObject.FindProperty("resourceSampleInterval");
            _enableGPUAnalysis = serializedObject.FindProperty("enableGPUAnalysis");
            _customMarkerNames = serializedObject.FindProperty("customMarkerNames");
        }

        public override void OnInspectorGUI()
        {
            serializedObject.Update();

            // Estimated data size
            EditorGUILayout.HelpBox(
                "Estimated base data rate: ~235 bytes/frame × 60fps ≈ 14KB/s ≈ 4.0MB per 5 minutes\n" +
                "Screenshots, overdraw heatmaps, logs and deep profiling samples are additional overhead.",
                MessageType.Info);

            if (!_enableDeepProfiling.boolValue)
            {
                EditorGUILayout.HelpBox(
                    "Deep Profiling is disabled. Desktop module pages and call stack analysis will not have function sampling data.",
                    MessageType.Warning);
            }

            if (GUILayout.Button("Apply Recommended Analysis Defaults"))
            {
                _enableDeepProfiling.boolValue = true;
                _captureLogs.boolValue = true;
                _deepProfilingSampleRate.intValue = 1;
            }

            EditorGUILayout.Space();
            EditorGUILayout.LabelField("General", EditorStyles.boldLabel);
            EditorGUILayout.PropertyField(_targetFps);
            EditorGUILayout.PropertyField(_sampleEveryNFrames);

            EditorGUILayout.Space();
            EditorGUILayout.LabelField("Modules", EditorStyles.boldLabel);
            EditorGUILayout.PropertyField(_enableMemory);
            EditorGUILayout.PropertyField(_enableRendering);
            EditorGUILayout.PropertyField(_enableModuleTiming);
            EditorGUILayout.PropertyField(_enableJankDetection);
            EditorGUILayout.PropertyField(_enableDeviceMetrics);
            EditorGUILayout.PropertyField(_enableScreenshots);
            EditorGUILayout.PropertyField(_enableOverdraw);

            if (_enableScreenshots.boolValue)
            {
                EditorGUILayout.Space();
                EditorGUILayout.LabelField("Screenshots", EditorStyles.boldLabel);
                EditorGUI.indentLevel++;
                EditorGUILayout.PropertyField(_screenshotInterval);
                EditorGUILayout.PropertyField(_screenshotThumbnailHeight);
                EditorGUILayout.PropertyField(_screenshotJpegQuality);
                EditorGUILayout.PropertyField(_fpsDropScreenshotThreshold);
                EditorGUI.indentLevel--;
            }

            if (_enableOverdraw.boolValue)
            {
                EditorGUILayout.Space();
                EditorGUILayout.LabelField("Overdraw", EditorStyles.boldLabel);
                EditorGUI.indentLevel++;
                EditorGUILayout.PropertyField(_overdrawSampleInterval);
                EditorGUILayout.PropertyField(_overdrawShader);
                EditorGUI.indentLevel--;
            }

            EditorGUILayout.Space();
            EditorGUILayout.LabelField("Network", EditorStyles.boldLabel);
            EditorGUILayout.PropertyField(_enableWifiTransfer);
            if (_enableWifiTransfer.boolValue)
            {
                EditorGUI.indentLevel++;
                EditorGUILayout.PropertyField(_httpServerPort);
                EditorGUI.indentLevel--;
            }

            EditorGUILayout.Space();
            EditorGUILayout.LabelField("Auto-Start", EditorStyles.boldLabel);
            EditorGUILayout.PropertyField(_autoStartCapture);
            if (_autoStartCapture.boolValue)
            {
                EditorGUI.indentLevel++;
                EditorGUILayout.PropertyField(_autoStartSessionName);
                EditorGUI.indentLevel--;
            }

            EditorGUILayout.Space();
            EditorGUILayout.LabelField("Deep Profiling", EditorStyles.boldLabel);
            EditorGUILayout.PropertyField(_enableDeepProfiling);
            EditorGUI.indentLevel++;
            using (new EditorGUI.DisabledScope(!_enableDeepProfiling.boolValue))
            {
                EditorGUILayout.PropertyField(_deepProfilingSampleRate);
            }
            EditorGUI.indentLevel--;

            EditorGUILayout.Space();
            EditorGUILayout.LabelField("Logs", EditorStyles.boldLabel);
            EditorGUILayout.PropertyField(_captureLogs);

            EditorGUILayout.Space();
            EditorGUILayout.LabelField("Advanced v3", EditorStyles.boldLabel);
            EditorGUILayout.PropertyField(_enableResourceMemory);
            if (_enableResourceMemory.boolValue)
            {
                EditorGUI.indentLevel++;
                EditorGUILayout.PropertyField(_resourceSampleInterval);
                EditorGUI.indentLevel--;
            }
            EditorGUILayout.PropertyField(_enableGPUAnalysis);
            EditorGUILayout.PropertyField(_customMarkerNames, true);

            serializedObject.ApplyModifiedProperties();
        }
    }
}
