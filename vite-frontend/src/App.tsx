import { Route, Routes, useNavigate } from "react-router-dom";
import { useEffect, useState } from "react";
import { Loader2 } from "lucide-react";

import IndexPage from "@/pages/index";
import ChangePasswordPage from "@/pages/change-password";
import DashboardPage from "@/pages/dashboard";
import ForwardPage from "@/pages/forward";
import TunnelPage from "@/pages/tunnel";
import NodePage from "@/pages/node";
import NodeStatusPage from "@/pages/node-status";
import UserPage from "@/pages/user";
import ProfilePage from "@/pages/profile";
import LimitPage from "@/pages/limit";
import ConfigPage from "@/pages/config";
import { SettingsPage } from "@/pages/settings";

import AdminLayout from "@/layouts/admin";
import H5Layout from "@/layouts/h5";
import H5SimpleLayout from "@/layouts/h5-simple";

import { isLoggedIn } from "@/utils/auth";
import { siteConfig } from "@/config/site";
import { useIsMobile } from "@/hooks/use-media-query";

const useH5Mode = () => {
  const isMobile = useIsMobile();
  const [isMobileBrowser] = useState(() => /Android|webOS|iPhone|iPad|iPod|BlackBerry|IEMobile|Opera Mini/i.test(navigator.userAgent));
  const [isH5Param] = useState(() => new URLSearchParams(window.location.search).get("h5") === "true");
  return isMobile || isMobileBrowser || isH5Param;
};

const LoadingScreen = () => (
  <div className="flex min-h-screen items-center justify-center">
    <div className="app-bg" />
    <Loader2 size={22} className="animate-spin text-blue-300" />
  </div>
);

const ProtectedRoute = ({ children, useSimpleLayout = false, skipLayout = false }: { children: React.ReactNode; useSimpleLayout?: boolean; skipLayout?: boolean }) => {
  const authenticated = isLoggedIn();
  const isH5 = useH5Mode();
  const navigate = useNavigate();

  useEffect(() => {
    if (!authenticated) navigate("/", { replace: true });
  }, [authenticated, navigate]);

  if (!authenticated) return <LoadingScreen />;
  if (skipLayout) return <>{children}</>;

  const Layout = isH5 ? (useSimpleLayout ? H5SimpleLayout : H5Layout) : AdminLayout;
  return <Layout>{children}</Layout>;
};

const LoginRoute = () => {
  const authenticated = isLoggedIn();
  const navigate = useNavigate();
  useEffect(() => {
    if (authenticated) navigate("/dashboard", { replace: true });
  }, [authenticated, navigate]);
  if (authenticated) return <LoadingScreen />;
  return <IndexPage />;
};

function App() {
  useEffect(() => {
    document.title = siteConfig.name;
    document.documentElement.classList.add("dark");
    const timer = setTimeout(async () => {
      try {
        const { getCachedConfig } = await import("@/config/site");
        const cached = await getCachedConfig("app_name");
        if (cached) document.title = cached;
      } catch { /* ignore */ }
    }, 100);
    return () => clearTimeout(timer);
  }, []);

  return (
    <Routes>
      <Route path="/" element={<LoginRoute />} />
      <Route path="/change-password" element={<ProtectedRoute skipLayout><ChangePasswordPage /></ProtectedRoute>} />
      <Route path="/dashboard" element={<ProtectedRoute><DashboardPage /></ProtectedRoute>} />
      <Route path="/forward" element={<ProtectedRoute><ForwardPage /></ProtectedRoute>} />
      <Route path="/tunnel" element={<ProtectedRoute><TunnelPage /></ProtectedRoute>} />
      <Route path="/node" element={<ProtectedRoute><NodePage /></ProtectedRoute>} />
      <Route path="/node-status" element={<ProtectedRoute><NodeStatusPage /></ProtectedRoute>} />
      <Route path="/user" element={<ProtectedRoute useSimpleLayout><UserPage /></ProtectedRoute>} />
      <Route path="/profile" element={<ProtectedRoute><ProfilePage /></ProtectedRoute>} />
      <Route path="/limit" element={<ProtectedRoute useSimpleLayout><LimitPage /></ProtectedRoute>} />
      <Route path="/config" element={<ProtectedRoute useSimpleLayout><ConfigPage /></ProtectedRoute>} />
      <Route path="/settings" element={<SettingsPage />} />
    </Routes>
  );
}

export default App;
