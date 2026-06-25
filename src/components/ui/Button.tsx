import React from "react";

interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?:
    | "primary"
    | "primary-soft"
    | "secondary"
    | "danger"
    | "danger-ghost"
    | "ghost";
  size?: "sm" | "md" | "lg";
}

export const Button: React.FC<ButtonProps> = ({
  children,
  className = "",
  variant = "primary",
  size = "md",
  ...props
}) => {
  const baseClasses =
    "font-medium rounded-xl border transition-all duration-150 cursor-pointer active:translate-y-[0.5px] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-background-ui/60 disabled:opacity-50 disabled:cursor-not-allowed disabled:active:translate-y-0";

  const variantClasses = {
    primary:
      "text-white bg-background-ui border-background-ui/70 shadow-[0_6px_20px_rgba(79,100,216,0.35)] hover:bg-background-ui/85 hover:border-background-ui/60",
    "primary-soft":
      "text-text bg-logo-primary/20 border-transparent hover:bg-logo-primary/30",
    secondary:
      "glass glass-interactive text-text border-transparent",
    danger:
      "text-white bg-red-600 border-mid-gray/20 hover:bg-red-700 hover:border-red-700 focus-visible:ring-red-500/60",
    "danger-ghost":
      "text-red-400 border-transparent hover:text-red-300 hover:bg-red-500/10 focus-visible:ring-red-500/60",
    ghost:
      "text-current border-transparent hover:bg-mid-gray/10 hover:border-logo-primary",
  };

  const sizeClasses = {
    sm: "px-2 py-1 text-xs",
    md: "px-4 py-[5px] text-sm",
    lg: "px-4 py-2 text-base",
  };

  return (
    <button
      className={`${baseClasses} ${variantClasses[variant]} ${sizeClasses[size]} ${className}`}
      {...props}
    >
      {children}
    </button>
  );
};
