// Deprecated: role-based routing removed in V3
import { Navigate } from 'react-router-dom';

export default function RoleSelect() {
  return <Navigate to="/overview" replace />;
}
