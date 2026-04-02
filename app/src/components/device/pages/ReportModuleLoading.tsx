import React from 'react';
import ModulePage from '../ModulePage';

export const ReportModuleLoading: React.FC<{ filePath: string }> = ({ filePath }) => (
  <ModulePage filePath={filePath} moduleName="loading" />
);
export default ReportModuleLoading;
