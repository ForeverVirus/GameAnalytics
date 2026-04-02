import React from 'react';
import ModulePage from '../ModulePage';

export const ReportModuleScripting: React.FC<{ filePath: string }> = ({ filePath }) => (
  <ModulePage filePath={filePath} moduleName="scripting" />
);
export default ReportModuleScripting;
