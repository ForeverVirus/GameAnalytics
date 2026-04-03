import type { CallTreeNode, DeviceProfileReport, ModulePageAnalysis, PerFrameFunctions, ReportMeta, ResourceMemoryAnalysis } from '../api/tauri';

const moduleAnalysisCache = new Map<string, ModulePageAnalysis>();
const callTreeCache = new Map<string, CallTreeNode[]>();
const resourceMemoryCache = new Map<string, ResourceMemoryAnalysis>();
const frameFunctionsCache = new Map<string, PerFrameFunctions>();
const screenshotCache = new Map<string, string>();
const reportHistoryCache = new Map<string, ReportMeta[]>();
let currentDeviceReportView: {
  tab: 'device' | 'report';
  reportPage: string;
  filePath: string;
  report: DeviceProfileReport | null;
} | null = null;

const moduleKey = (filePath: string, moduleName: string) => `${filePath}::${moduleName}`;
const callTreeKey = (filePath: string, direction: 'forward' | 'reverse') => `${filePath}::${direction}`;
const frameFunctionsKey = (filePath: string, frameIndex: number, categoryFilters?: number[], preferNearest?: boolean) =>
  `${filePath}::frame::${frameIndex}::${categoryFilters?.join(',') ?? '*'}::${preferNearest ? 'nearest' : 'exact'}`;
const screenshotKey = (filePath: string, frameIndex: number) => `${filePath}::screenshot::${frameIndex}`;
const reportHistoryKey = (projectPath?: string) => projectPath || '__global__';

export function getCachedModuleAnalysis(filePath: string, moduleName: string) {
  return moduleAnalysisCache.get(moduleKey(filePath, moduleName));
}

export function setCachedModuleAnalysis(filePath: string, moduleName: string, analysis: ModulePageAnalysis) {
  moduleAnalysisCache.set(moduleKey(filePath, moduleName), analysis);
}

export function getCachedCallTree(filePath: string, direction: 'forward' | 'reverse') {
  return callTreeCache.get(callTreeKey(filePath, direction));
}

export function setCachedCallTree(filePath: string, direction: 'forward' | 'reverse', nodes: CallTreeNode[]) {
  callTreeCache.set(callTreeKey(filePath, direction), nodes);
}

export function getCachedResourceMemoryAnalysis(filePath: string) {
  return resourceMemoryCache.get(filePath);
}

export function setCachedResourceMemoryAnalysis(filePath: string, analysis: ResourceMemoryAnalysis) {
  resourceMemoryCache.set(filePath, analysis);
}

export function getCachedFrameFunctions(filePath: string, frameIndex: number, categoryFilters?: number[], preferNearest?: boolean) {
  return frameFunctionsCache.get(frameFunctionsKey(filePath, frameIndex, categoryFilters, preferNearest));
}

export function setCachedFrameFunctions(filePath: string, frameIndex: number, data: PerFrameFunctions, categoryFilters?: number[], preferNearest?: boolean) {
  frameFunctionsCache.set(frameFunctionsKey(filePath, frameIndex, categoryFilters, preferNearest), data);
}

export function getCachedScreenshot(filePath: string, frameIndex: number) {
  return screenshotCache.get(screenshotKey(filePath, frameIndex));
}

export function setCachedScreenshot(filePath: string, frameIndex: number, imageData: string) {
  screenshotCache.set(screenshotKey(filePath, frameIndex), imageData);
}

export function getCachedReportHistory(projectPath?: string) {
  return reportHistoryCache.get(reportHistoryKey(projectPath));
}

export function setCachedReportHistory(projectPath: string | undefined, reports: ReportMeta[]) {
  reportHistoryCache.set(reportHistoryKey(projectPath), reports);
}

export function clearCachedReportHistory(projectPath?: string) {
  reportHistoryCache.delete(reportHistoryKey(projectPath));
}

export function getCachedDeviceReportView() {
  return currentDeviceReportView;
}

export function setCachedDeviceReportView(view: {
  tab: 'device' | 'report';
  reportPage: string;
  filePath: string;
  report: DeviceProfileReport | null;
}) {
  currentDeviceReportView = view;
}
