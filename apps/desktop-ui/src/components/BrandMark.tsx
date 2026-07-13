export function BrandMark({ size = 24 }: { size?: number }) {
  return (
    <svg
      aria-hidden="true"
      className="brand-mark"
      height={size}
      viewBox="0 0 24 24"
      width={size}
    >
      <path d="M12 1.5 20 6l-8 4.5L4 6l8-4.5Z" fill="currentColor" />
      <path d="m4 10 4 2.25L12 10l4 2.25L12 14.5 4 10Z" fill="currentColor" opacity="0.72" />
      <path d="m4 14 8 4.5 8-4.5v4L12 22.5 4 18v-4Z" fill="currentColor" opacity="0.92" />
    </svg>
  );
}
