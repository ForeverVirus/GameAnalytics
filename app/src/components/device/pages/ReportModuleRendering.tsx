import React from 'react';
import ModulePage from '../ModulePage';

export const ReportModuleRendering: React.FC<{ filePath: string }> = ({ filePath }) => (
  <ModulePage filePath={filePath} moduleName="rendering" />
);
export default ReportModuleRendering;
