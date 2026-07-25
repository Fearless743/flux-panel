import {heroui} from "@heroui/theme"

/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    './src/layouts/**/*.{js,ts,jsx,tsx,mdx}',
    './src/pages/**/*.{js,ts,jsx,tsx,mdx}',
    './src/components/**/*.{js,ts,jsx,tsx,mdx}',
    "./node_modules/@heroui/theme/dist/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      fontFamily: {
        sans: [
          "IBM Plex Sans",
          "Noto Sans SC",
          "ui-sans-serif",
          "system-ui",
          "-apple-system",
          "Segoe UI",
          "sans-serif",
        ],
        mono: [
          "IBM Plex Mono",
          "ui-monospace",
          "SFMono-Regular",
          "Menlo",
          "monospace",
        ],
      },
      boxShadow: {
        panel: "0 1px 2px rgb(15 23 42 / 0.04), 0 8px 24px rgb(15 23 42 / 0.06)",
        "panel-lg": "0 2px 4px rgb(15 23 42 / 0.04), 0 16px 40px rgb(15 23 42 / 0.08)",
      },
      backgroundImage: {
        "login-mesh":
          "radial-gradient(ellipse 80% 60% at 20% 10%, rgba(14, 165, 164, 0.18), transparent 55%), radial-gradient(ellipse 70% 50% at 90% 20%, rgba(56, 189, 248, 0.14), transparent 50%), radial-gradient(ellipse 60% 40% at 50% 100%, rgba(99, 102, 241, 0.10), transparent 55%)",
        "login-mesh-dark":
          "radial-gradient(ellipse 80% 60% at 20% 10%, rgba(45, 212, 191, 0.12), transparent 55%), radial-gradient(ellipse 70% 50% at 90% 20%, rgba(56, 189, 248, 0.10), transparent 50%), radial-gradient(ellipse 60% 40% at 50% 100%, rgba(129, 140, 248, 0.08), transparent 55%)",
      },
    },
  },
  darkMode: "class",
  plugins: [
    heroui({
      themes: {
        light: {
          colors: {
            background: "#f4f7fb",
            foreground: "#0f172a",
            content1: "#ffffff",
            content2: "#f1f5f9",
            content3: "#e2e8f0",
            content4: "#cbd5e1",
            divider: "rgba(15, 23, 42, 0.08)",
            focus: "#0d9488",
            primary: {
              50: "#f0fdfa",
              100: "#ccfbf1",
              200: "#99f6e4",
              300: "#5eead4",
              400: "#2dd4bf",
              500: "#14b8a6",
              600: "#0d9488",
              700: "#0f766e",
              800: "#115e59",
              900: "#134e4a",
              DEFAULT: "#0d9488",
              foreground: "#ffffff",
            },
            secondary: {
              50: "#eff6ff",
              100: "#dbeafe",
              200: "#bfdbfe",
              300: "#93c5fd",
              400: "#60a5fa",
              500: "#3b82f6",
              600: "#2563eb",
              700: "#1d4ed8",
              800: "#1e40af",
              900: "#1e3a8a",
              DEFAULT: "#2563eb",
              foreground: "#ffffff",
            },
          },
        },
        dark: {
          colors: {
            background: "#0b1220",
            foreground: "#e8eef9",
            content1: "#111a2b",
            content2: "#172235",
            content3: "#1e2b42",
            content4: "#2a3a55",
            divider: "rgba(148, 163, 184, 0.14)",
            focus: "#2dd4bf",
            primary: {
              50: "#134e4a",
              100: "#115e59",
              200: "#0f766e",
              300: "#0d9488",
              400: "#14b8a6",
              500: "#2dd4bf",
              600: "#5eead4",
              700: "#99f6e4",
              800: "#ccfbf1",
              900: "#f0fdfa",
              DEFAULT: "#2dd4bf",
              foreground: "#042f2e",
            },
            secondary: {
              50: "#1e3a8a",
              100: "#1e40af",
              200: "#1d4ed8",
              300: "#2563eb",
              400: "#3b82f6",
              500: "#60a5fa",
              600: "#93c5fd",
              700: "#bfdbfe",
              800: "#dbeafe",
              900: "#eff6ff",
              DEFAULT: "#60a5fa",
              foreground: "#0b1220",
            },
          },
        },
      },
      layout: {
        radius: {
          small: "0.5rem",
          medium: "0.75rem",
          large: "1rem",
        },
        borderWidth: {
          small: "1px",
          medium: "1.5px",
          large: "2px",
        },
      },
    }),
  ],
}
