// nextstepjs ships a Next.js adapter that imports `next/navigation`. This app uses
// its React Router adapter instead, so Vite aliases that import here (see
// vite.config.ts). Nothing calls these at runtime.
export const useRouter = () => ({
  push: () => {},
  replace: () => {},
  prefetch: () => {},
  back: () => {},
  forward: () => {},
  refresh: () => {},
});
export const usePathname = () => "";
export const useSearchParams = () => new URLSearchParams();
export const useParams = () => ({});
