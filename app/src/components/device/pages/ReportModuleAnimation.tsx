import React from 'react';
import ModulePage from '../ModulePage';

export const ReportModuleAnimation: React.FC<{ filePath: string }> = ({ filePath }) => (
  <ModulePage filePath={filePath} moduleName="animation" />
);
export default ReportModuleAnimation;
