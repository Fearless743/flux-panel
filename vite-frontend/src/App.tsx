import { Route, Routes, Navigate, useNavigate } from "react-router-dom";
import { useEffect, useState } from "react";

import IndexPage from "@/pages/index";
import ChangePasswordPage from "@/pages/change-password";
import DashboardPage from "@/pages/dashboard";
import ForwardPage from "@/pages/forward";
import TunnelPage from "@/pages/tunnel";
import NodePage from "@/pages/node";
import UserPage from "@/pages/user";
import LimitPage from "@/pages/limit";
import PageWrapper from "@/components/page-wrapper";
import { isLoggedIn } from "@/utils/auth";

// 路由保护组件 - 使用useNavigate避免循环渲染
const ProtectedRoute = ({ children }: { children: React.ReactNode }) => {
  const [checking, setChecking] = useState(true);
  const [authenticated, setAuthenticated] = useState(false);
  const navigate = useNavigate();

  useEffect(() => {
    const checkAuth = () => {
      const loggedIn = isLoggedIn();
      
      if (!loggedIn) {
        // 如果未登录，直接导航到登录页
        navigate('/', { replace: true });
        return;
      }
      
      setAuthenticated(true);
      setChecking(false);
    };

    checkAuth();
  }, [navigate]);

  if (checking) {
    return (
      <div className="flex items-center justify-center min-h-screen">
        <div className="text-lg">检查登陆状态...</div>
      </div>
    );
  }

  return authenticated ? <>{children}</> : null;
};

function App() {
  return (
    <Routes>
      <Route 
        path="/" 
        element={
          isLoggedIn() ? <Navigate to="/dashboard" replace /> : <IndexPage />
        } 
      />
      <Route 
        path="/change-password" 
        element={
          <ProtectedRoute>
            <ChangePasswordPage />
          </ProtectedRoute>
        } 
      />
      <Route 
        path="/dashboard" 
        element={
          <ProtectedRoute>
            <DashboardPage />
          </ProtectedRoute>
        } 
      />
      <Route 
        path="/forward" 
        element={
          <ProtectedRoute>
            <ForwardPage />
          </ProtectedRoute>
        } 
      />
      <Route 
        path="/tunnel" 
        element={
          <ProtectedRoute>
            <TunnelPage />
          </ProtectedRoute>
        } 
      />
      <Route 
        path="/node" 
        element={
          <ProtectedRoute>
            <NodePage />
          </ProtectedRoute>
        } 
      />
      <Route 
        path="/user" 
        element={
          <ProtectedRoute>
            <UserPage />
          </ProtectedRoute>
        } 
      />
      <Route 
        path="/limit" 
        element={
          <ProtectedRoute>
            <LimitPage />
          </ProtectedRoute>
        } 
      />
    </Routes>
  );
}

export default App;
