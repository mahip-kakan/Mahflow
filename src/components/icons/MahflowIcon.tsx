const MahflowIcon = ({
  width,
  height,
  className,
}: {
  width?: number | string;
  height?: number | string;
  className?: string;
}) => (
  <svg
    width={width || 24}
    height={height || 24}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth={2}
    strokeLinecap="round"
    strokeLinejoin="round"
    className={className}
    xmlns="http://www.w3.org/2000/svg"
    aria-hidden="true"
  >
    {/* Sound-wave lines flowing in from the left… */}
    <path d="M2 8c2.2 0 3.4-1 5-1" opacity="0.9" />
    <path d="M2 12h5" opacity="0.9" />
    <path d="M2 16c2.2 0 3.4 1 5 1" opacity="0.9" />
    {/* …merging into a clean monogram "M". */}
    <path d="M9.5 18V6.5l4 6 4-6V18" />
  </svg>
);

export default MahflowIcon;
