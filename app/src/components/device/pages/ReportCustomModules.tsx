import React from 'react';
import ModulePage from '../ModulePage';

export const ReportCustomModules: React.FC<{ filePath: string }> = ({ filePath }) => (
  <ModulePage filePath={filePath} moduleName="custom" />
);

export default ReportCustomModules;
