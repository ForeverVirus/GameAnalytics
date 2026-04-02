import React from 'react';
import ModulePage from '../ModulePage';

export const ReportModuleParticle: React.FC<{ filePath: string }> = ({ filePath }) => (
  <ModulePage filePath={filePath} moduleName="particles" />
);
export default ReportModuleParticle;
