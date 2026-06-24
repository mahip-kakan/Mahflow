import React from "react";

const MahflowTextLogo = ({
  width,
  height,
  className,
}: {
  width?: number;
  height?: number;
  className?: string;
}) => {
  return (
    <svg
      width={width ?? 220}
      height={height ?? 48}
      className={className}
      viewBox="0 0 220 48"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      role="img"
      aria-label="Mahflow"
    >
      <text
        x="0"
        y="36"
        className="fill-text"
        style={{
          fontFamily:
            'ui-sans-serif, system-ui, -apple-system, "Segoe UI", sans-serif',
          fontSize: "36px",
          fontWeight: 700,
          letterSpacing: "-0.02em",
        }}
      >
        Mahflow
      </text>
    </svg>
  );
};

export default MahflowTextLogo;
