import React from 'react';
import ModulePage from '../ModulePage';

interface ReportGPUProps {
  filePath: string;
}

export const ReportGPU: React.FC<ReportGPUProps> = ({ filePath }) => (
  <ModulePage filePath={filePath} moduleName="gpu" />
);

export default ReportGPU;
