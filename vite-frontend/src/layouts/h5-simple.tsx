import React from 'react';
import { useNavigate, useLocation } from 'react-router-dom';
import { Button } from "@heroui/button";

import { Logo } from '@/components/icons';
import { siteConfig } from '@/config/site';

export default function H5SimpleLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  const navigate = useNavigate();
  const location = useLocation();

  // 路由切换时回到顶部，避免上一页滚动位置保留
  React.useEffect(() => {
    try {
      window.scrollTo({ top: 0, left: 0, behavior: 'auto' });
    } catch (e) {
      window.scrollTo(0, 0);
    }
    document.body.scrollTop = 0;
    document.documentElement.scrollTop = 0;
  }, [location.pathname]);

  const handleBack = () => {
    navigate('/profile');
  };

  return (
    <div className="flex flex-col min-h-screen bg-background">
      {/* 顶部导航栏 */}
      <header className="bg-content1/95 backdrop-blur-md border-b border-divider h-14 safe-top flex-shrink-0 flex items-center justify-between px-4 relative z-10">
        <div className="flex items-center gap-2">
          <Button
            isIconOnly
            variant="light"
            size="sm"
            onPress={handleBack}
            aria-label="返回"
          >
            <svg className="w-5 h-5" fill="currentColor" viewBox="0 0 20 20">
              <path fillRule="evenodd" d="M12.707 5.293a1 1 0 010 1.414L9.414 10l3.293 3.293a1 1 0 01-1.414 1.414l-4-4a1 1 0 010-1.414l4-4a1 1 0 011.414 0z" clipRule="evenodd" />
            </svg>
          </Button>
          <div className="flex h-7 w-7 items-center justify-center rounded-md bg-primary/10 text-primary">
            <Logo size={16} />
          </div>
          <h1 className="text-sm font-semibold text-foreground tracking-tight">{siteConfig.name}</h1>
        </div>

        <div className="flex items-center gap-2" />
      </header>

      {/* 主内容区域 */}
      <main className="flex-1 bg-background pb-0">
        {children}
      </main>
    </div>
  );
}
