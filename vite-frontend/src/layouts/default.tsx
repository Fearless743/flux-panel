import { Zap } from "lucide-react";
import { siteConfig } from "@/config/site";

export default function DefaultLayout({ children }: { children: React.ReactNode }) {
  return (
    <div className="relative flex min-h-screen flex-col">
      <div className="app-bg" />
      <header className="flex h-14 items-center gap-2.5 px-5">
        <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-blue-600">
          <Zap size={15} className="text-white" />
        </div>
        <span className="text-[0.9rem] font-bold tracking-tight text-white">{siteConfig.name}</span>
        <span className="mono ml-auto text-[0.7rem] text-slate-500">v{siteConfig.version}</span>
      </header>
      <main className="mx-auto w-full max-w-7xl flex-1 px-4 sm:px-6">{children}</main>
    </div>
  );
}
