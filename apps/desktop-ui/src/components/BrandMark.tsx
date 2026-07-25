export function BrandMark({ size = 24 }: { size?: number }) {
  return (
    <svg
      aria-hidden="true"
      className="brand-mark"
      height={size}
      viewBox="0 0 32 32"
      width={size}
    >
      <path
        d="M3 6h26v2.2c0 2.2-2.7 4.3-6.4 5.1h-3.4l-2 3.2v3.7h4.3V26h-11v-5.8h4.3v-3.7l-2.3-3.2H8.2C5.4 12.3 3 10.3 3 8.2V6Z"
        fill="currentColor"
      />
      <path
        d="m10.8 8.6 2.2 2.1-2.2 2.1m10.4-4.2L19 10.7l2.2 2.1m-5.3-4.8-1.6 5.4"
        fill="none"
        stroke="var(--color-accent, #e9a07c)"
        strokeLinecap="round"
        strokeLinejoin="round"
        strokeWidth="1.55"
      />
    </svg>
  );
}
