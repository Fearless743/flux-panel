import { useNavigate } from "react-router-dom";
import { ArrowLeft } from "lucide-react";
import { siteConfig } from "@/config/site";

export default function H5SimpleLayout({ children }: { children: React.ReactNode }) {
  const navigate = useNavigate();
  return (
    <div className="min-h-screen">
      <div className="app-bg" />
      <header className="safe-top fixed inset-x-0 top-0 z-40 border-b border-white/[0.07] bg-[#0b0e13]">
        <div className="flex h-14 items-center gap-2 px-4">
          <button onClick={() => navigate(-1)} className="rounded-lg border border-white/10 bg-white/[0.04] p-2 text-slate-300" aria-label="返回">
            <ArrowLeft size={16} />
          </button>
          <p className="truncate text-tmd font-bold text-white">{siteConfig.name}</p>
        </div>
      </header>
      <main className="mx-auto w-full max-w-[1200px] px-4 pb-12 pt-20">
        <div className="page-enter">{children}</div>
      </main>
    </div>
  );
}
