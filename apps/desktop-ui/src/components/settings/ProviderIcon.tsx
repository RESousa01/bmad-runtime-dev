import type { SVGProps } from "react";

/**
 * Provider marks adapted from OpenCode's MIT-licensed provider icon sprite.
 * Source: anomalyco/opencode, packages/ui/src/components/provider-icons/sprite.svg
 */
export function ProviderIcon({
  provider,
  ...props
}: SVGProps<SVGSVGElement> & { readonly provider: "azure" | "openai" }) {
  return (
    <svg
      {...props}
      aria-hidden={props["aria-hidden"] ?? true}
      fill="none"
      viewBox="0 0 40 40"
      xmlns="http://www.w3.org/2000/svg"
    >
      {provider === "azure" ? (
        <path
          d="M21.68 7.584 11.296 29.68 4 29.6l8.144-14.016L21.68 7.584ZM22.8 9.328 36 32.416H11.584l14.88-2.656-7.792-9.264L22.8 9.328Z"
          fill="currentColor"
        />
      ) : (
        <path
          d="M32.84 17.28a8.02 8.02 0 0 0-.56-6.09 7.79 7.79 0 0 0-8.16-3.66A7.83 7.83 0 0 0 18.5 5a7.8 7.8 0 0 0-7.22 5.16 8.04 8.04 0 0 0-5.06 3.66 7.78 7.78 0 0 0 .93 8.9 7.92 7.92 0 0 0 .56 6.09 7.8 7.8 0 0 0 8.16 3.66A7.83 7.83 0 0 0 21.5 35a7.8 7.8 0 0 0 7.21-5.16 8.02 8.02 0 0 0 4.97-3.65 7.79 7.79 0 0 0-.84-8.91Zm-11.25 15.75a5.7 5.7 0 0 1-3.66-1.31l6.19-3.57a.83.83 0 0 0 .47-.84v-8.44l2.53 1.5v6.94a5.53 5.53 0 0 1-5.53 5.72ZM9.5 27.87a5.7 5.7 0 0 1-.66-3.75l6.18 3.57a.84.84 0 0 0 .94 0l7.31-4.22v2.9l-6.09 3.57a5.54 5.54 0 0 1-7.68-2.07Zm-1.6-13.03a5.7 5.7 0 0 1 2.91-2.43v7.12a.84.84 0 0 0 .47.84l7.31 4.22-2.53 1.5-6-3.47a5.54 5.54 0 0 1-2.16-7.78Zm20.72 4.79-7.31-4.22 2.53-1.5 6 3.47a5.54 5.54 0 0 1-.85 10.12v-7.13a.84.84 0 0 0-.37-.74Zm2.53-3.75-6.19-3.57a.84.84 0 0 0-.94 0l-7.31 4.22v-2.91l6.09-3.56a5.54 5.54 0 0 1 8.35 5.82ZM15.4 21.12l-2.53-1.5v-7.03a5.54 5.54 0 0 1 9.19-4.31l-6.19 3.56a.84.84 0 0 0-.47.84v8.44Zm1.31-3 3.28-1.87 3.28 1.87v3.75l-3.28 1.88-3.28-1.88v-3.75Z"
          fill="currentColor"
        />
      )}
    </svg>
  );
}
