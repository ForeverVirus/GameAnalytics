import React from 'react';
import type { DeviceProfileReport } from '../../../api/tauri';
import ScreenshotViewer from '../ScreenshotViewer';

interface ReportScreenshotsProps {
  filePath: string;
  report: DeviceProfileReport;
}

export const ReportScreenshots: React.FC<ReportScreenshotsProps> = ({ filePath, report }) => {
  if (report.screenshot_count === 0) {
    return <div style={{ padding: 40, textAlign: 'center', color: '#888' }}>无截图数据</div>;
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
      <h3 style={{ margin: 0, color: '#e0e0e0', fontSize: 16 }}>截图浏览 ({report.screenshot_count} 帧)</h3>
      <ScreenshotViewer
        filePath={filePath}
        totalFrames={report.total_frames}
        availableFrames={report.screenshot_frame_indices}
        currentFrame={report.screenshot_frame_indices[0] ?? 0}
      />
    </div>
  );
};

export default ReportScreenshots;
