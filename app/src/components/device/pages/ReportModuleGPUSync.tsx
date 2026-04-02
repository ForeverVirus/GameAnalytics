import React from 'react';
import ModulePage from '../ModulePage';

export const ReportModuleGPUSync: React.FC<{ filePath: string }> = ({ filePath }) => (
  <ModulePage filePath={filePath} moduleName="gpu_sync" />
);
export default ReportModuleGPUSync;
