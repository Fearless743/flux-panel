import type { NavigateOptions } from "react-router-dom";

import * as React from "react";
import { HeroUIProvider } from "@heroui/system";
import { useHref, useNavigate } from "react-router-dom";
import { Toaster } from "react-hot-toast";
import { I18nProvider } from "@react-aria/i18n";

import { ThemeProvider } from "@/components/theme-provider";

declare module "@react-types/shared" {
  interface RouterConfig {
    routerOptions: NavigateOptions;
  }
}

export interface ProvidersProps {
  children: React.ReactNode;
}

export function Provider({ children }: ProvidersProps) {
  const navigate = useNavigate();

  return (
    <I18nProvider locale="zh-CN">
      <HeroUIProvider navigate={navigate} useHref={useHref}>
        <ThemeProvider>
          {children}
          <Toaster
            position="top-center"
            toastOptions={{
              duration: 2000,
              className: "text-sm font-medium shadow-panel",
              style: {
                background: "var(--toaster-bg, #ffffff)",
                color: "var(--toaster-color, #0f172a)",
                border:
                  "1px solid var(--toaster-border, rgba(15, 23, 42, 0.1))",
                borderRadius: "0.75rem",
              },
              success: {
                duration: 2000,
                style: {
                  background: "#0d9488",
                  color: "#ffffff",
                  border: "none",
                },
              },
              error: {
                duration: 2000,
                style: {
                  background: "#ef4444",
                  color: "#ffffff",
                  border: "none",
                },
              },
            }}
          />
        </ThemeProvider>
      </HeroUIProvider>
    </I18nProvider>
  );
}
