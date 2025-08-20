import React from 'react';
import { useNavigate } from 'react-router-dom';
import { Button } from "@heroui/button";

import { Logo } from '@/components/icons';
import { siteConfig } from '@/config/site';

export default function H5SimpleLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  const navigate = useNavigate();

  const handleBack = () => {
    navigate('/profile');
  };

  return (
    <div className="flex flex-col h-screen bg-gray-100 dark:bg-black">
      {/* 顶部导航栏 */}
      <header className="bg-white dark:bg-black shadow-sm border-b border-gray-200 dark:border-gray-600 h-14 flex items-center justify-between px-4 relative z-10">
        <div className="flex items-center gap-2">
          <Button
            isIconOnly
            variant="light"
            size="sm"
            onPress={handleBack}
          >
            <svg className="w-5 h-5" fill="currentColor" viewBox="0 0 20 20">
              <path fillRule="evenodd" d="M12.707 5.293a1 1 0 010 1.414L9.414 10l3.293 3.293a1 1 0 01-1.414 1.414l-4-4a1 1 0 010-1.414l4-4a1 1 0 011.414 0z" clipRule="evenodd" />
            </svg>
          </Button>
          <Logo size={20} />
          <h1 className="text-sm font-bold text-foreground">{siteConfig.name}</h1>
        </div>

        <div className="flex items-center gap-2">
        </div>
      </header>

      {/* 主内容区域 */}
      <main className="flex-1 overflow-y-auto bg-gray-100 dark:bg-black">
        {children}
      </main>
    </div>
  );
}
