import React from 'react';
import ModulePage from '../ModulePage';

export const ReportModuleUI: React.FC<{ filePath: string }> = ({ filePath }) => (
  <ModulePage filePath={filePath} moduleName="ui" />
);
export default ReportModuleUI;
