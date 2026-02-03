import React from 'react';

interface PowerPasteLogoProps {
  size?: number;
  className?: string;
  classNameBolt?: string;
  colorful?: boolean;
}

export const PowerPasteLogo: React.FC<PowerPasteLogoProps> = ({ 
  size = 64, 
  className = "", 
  classNameBolt = "",
  colorful = true 
}) => {
  // Brand colors from MASTER.md
  const colorPrimary = "#0D9488";
  const colorSecondary = "#14B8A6";
  const colorAccent = "#F97316";
  const colorBg = "#F0FDFA"; // Very light teal/white
  const colorDark = "#134E4A";

  return (
    <svg 
      width={size} 
      height={size} 
      viewBox="0 0 512 512" 
      fill="none" 
      xmlns="http://www.w3.org/2000/svg"
      className={className}
    >
      <defs>
        <linearGradient id={`grad1-${size}`} x1="96" y1="64" x2="416" y2="448" gradientUnits="userSpaceOnUse">
          <stop offset="0%" stopColor={colorful ? colorSecondary : "currentColor"} />
          <stop offset="100%" stopColor={colorful ? colorPrimary : "currentColor"} />
        </linearGradient>
      </defs>
      
      {/* Clipboard Body */}
      <rect 
        x="96" y="64" 
        width="320" height="384" 
        rx="48" 
        fill={colorful ? `url(#grad1-${size})` : "currentColor"} 
        fillOpacity={colorful ? 1 : 0.8}
      />
      
      {/* Paper Sheet */}
      <rect 
        x="128" y="140" 
        width="256" height="280" 
        rx="24" 
        fill={colorful ? colorBg : "white"} 
        fillOpacity={colorful ? 0.95 : 0.4}
      />
      
      {/* Power Bolt Icon */}
      <path 
        d="M276 190 L230 280 H280 L250 370 L340 260 H290 L320 190 H276 Z" 
        fill={colorful ? colorAccent : "currentColor"}
        stroke={colorful ? colorBg : "transparent"} 
        strokeWidth="8" 
        strokeLinejoin="round" 
        className={classNameBolt}
      />
      
      {/* Clip Mechanism */}
      <rect 
        x="176" y="48" 
        width="160" height="64" 
        rx="20" 
        fill={colorful ? colorDark : "currentColor"}
      />
      <circle 
        cx="256" cy="80" 
        r="12" 
        fill={colorful ? colorBg : "white"} 
        fillOpacity={0.3}
      />
    </svg>
  );
};
