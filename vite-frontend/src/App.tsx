import { Route, Routes } from "react-router-dom";
import { useEffect } from "react";

import IndexPage from "@/pages/index";
import ChangePasswordPage from "@/pages/change-password";
import DashboardPage from "@/pages/dashboard";
import ForwardPage from "@/pages/forward";
import TunnelPage from "@/pages/tunnel";
import NodePage from "@/pages/node";
import UserPage from "@/pages/user";
import LimitPage from "@/pages/limit";

import { isLoggedIn } from "@/utils/auth";

// 简化的路由保护组件 - 使用浏览器重定向避免React循环
const ProtectedRoute = ({ children }: { children: React.ReactNode }) => {
  const authenticated = isLoggedIn();
  
  useEffect(() => {
    if (!authenticated) {
      // 使用浏览器原生重定向，避免React渲染循环
      window.location.replace('/');
    }
  }, [authenticated]);

  if (!authenticated) {
    return (
      <div className="flex items-center justify-center min-h-screen">
        <div className="text-lg">重定向中...</div>
      </div>
    );
  }

  return <>{children}</>;
};

// 登录页面路由组件 - 已登录则重定向到dashboard
const LoginRoute = () => {
  const authenticated = isLoggedIn();
  
  useEffect(() => {
    if (authenticated) {
      // 使用浏览器原生重定向
      window.location.replace('/dashboard');
    }
  }, [authenticated]);
  
  if (authenticated) {
    return (
      <div className="flex items-center justify-center min-h-screen">
        <div className="text-lg">重定向中...</div>
      </div>
    );
  }
  
  return <IndexPage />;
};

function App() {
  return (
    <Routes>
      <Route path="/" element={<LoginRoute />} />
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
