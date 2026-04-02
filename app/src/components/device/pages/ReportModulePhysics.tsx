import React from 'react';
import ModulePage from '../ModulePage';

export const ReportModulePhysics: React.FC<{ filePath: string }> = ({ filePath }) => (
  <ModulePage filePath={filePath} moduleName="physics" />
);
export default ReportModulePhysics;
