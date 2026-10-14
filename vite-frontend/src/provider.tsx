import { Toaster } from "react-hot-toast";

export function Provider({ children }: { children: React.ReactNode }) {
  return (
    <>
      {children}
      <Toaster
        position="top-center"
        toastOptions={{
          duration: 2200,
          style: {
            background: "rgba(21, 26, 35, .95)",
            color: "#e6edf7",
            border: "1px solid rgba(255,255,255,.12)",
            borderRadius: "0.8rem",
            fontSize: "0.84rem",
            backdropFilter: "blur(12px)",
          },
          success: {
            style: { background: "#15803d", color: "#fff", border: "none" },
          },
          error: {
            style: { background: "#b91c1c", color: "#fff", border: "none" },
          },
        }}
      />
    </>
  );
}
