import type { SVGProps } from "react";

type IconProps = SVGProps<SVGSVGElement> & { size?: number };

function Svg({ size = 16, children, ...props }: IconProps) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 20 20"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.6}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
      {...props}
    >
      {children}
    </svg>
  );
}

export function LogoMark({ size = 28, className = "" }: { size?: number; className?: string }) {
  return (
    <svg width={size} height={size} viewBox="0 0 28 28" aria-hidden="true" className={className}>
      <rect width="28" height="28" rx="7" fill="#1f3d32" />
      <path
        d="M14 21V14.6M14 14.6 9.2 9.8M14 14.6l4.8-4.8"
        stroke="#f6f5f2"
        strokeWidth="1.9"
        strokeLinecap="round"
        fill="none"
      />
      <circle cx="8.6" cy="9.2" r="2.2" fill="#f6f5f2" />
      <circle cx="19.4" cy="9.2" r="2.2" fill="#f6f5f2" />
      <circle cx="14" cy="21.2" r="2.2" fill="none" stroke="#f6f5f2" strokeWidth="1.6" />
    </svg>
  );
}

export function Logo({ suffix }: { suffix?: string }) {
  return (
    <span className="inline-flex items-center gap-2.5">
      <LogoMark size={26} />
      <span className="text-[15px] font-semibold tracking-tight text-ink">Knotree</span>
      {suffix ? (
        <span className="border-l border-line-strong pl-2.5 text-[15px] tracking-tight text-muted">{suffix}</span>
      ) : null}
    </span>
  );
}

export const HomeIcon = (p: IconProps) => (
  <Svg {...p}>
    <path d="M3.5 8.6 10 3.5l6.5 5.1V16a.5.5 0 0 1-.5.5h-3.6v-4.4H7.6v4.4H4a.5.5 0 0 1-.5-.5Z" />
  </Svg>
);
export const UserIcon = (p: IconProps) => (
  <Svg {...p}>
    <circle cx="10" cy="6.8" r="3.1" />
    <path d="M3.8 16.5c.9-2.8 3.3-4.4 6.2-4.4s5.3 1.6 6.2 4.4" />
  </Svg>
);
export const ShieldIcon = (p: IconProps) => (
  <Svg {...p}>
    <path d="M10 2.8 4 5v4.6c0 3.6 2.5 6.4 6 7.6 3.5-1.2 6-4 6-7.6V5Z" />
  </Svg>
);
export const ShieldCheckIcon = (p: IconProps) => (
  <Svg {...p}>
    <path d="M10 2.8 4 5v4.6c0 3.6 2.5 6.4 6 7.6 3.5-1.2 6-4 6-7.6V5Z" />
    <path d="m7.4 10 1.8 1.8 3.5-3.6" />
  </Svg>
);
export const DevicesIcon = (p: IconProps) => (
  <Svg {...p}>
    <rect x="2.5" y="4" width="11.5" height="8" rx="1" />
    <path d="M1.5 15h11" />
    <rect x="14.5" y="7.5" width="4" height="8" rx="0.8" />
  </Svg>
);
export const MonitorIcon = (p: IconProps) => (
  <Svg {...p}>
    <rect x="2.5" y="3.5" width="15" height="10" rx="1.2" />
    <path d="M7 16.5h6M10 13.5v3" />
  </Svg>
);
export const PhoneIcon = (p: IconProps) => (
  <Svg {...p}>
    <rect x="5.5" y="2.5" width="9" height="15" rx="1.6" />
    <path d="M9 14.8h2" />
  </Svg>
);
export const LinkIcon = (p: IconProps) => (
  <Svg {...p}>
    <path d="M8.5 11.5a3.2 3.2 0 0 0 4.6 0l2.4-2.4a3.2 3.2 0 0 0-4.6-4.6l-.9.9" />
    <path d="M11.5 8.5a3.2 3.2 0 0 0-4.6 0l-2.4 2.4a3.2 3.2 0 0 0 4.6 4.6l.9-.9" />
  </Svg>
);
export const LogOutIcon = (p: IconProps) => (
  <Svg {...p}>
    <path d="M8 16.5H4.5a1 1 0 0 1-1-1v-11a1 1 0 0 1 1-1H8" />
    <path d="M13 13.5 16.5 10 13 6.5M16.5 10H7.5" />
  </Svg>
);
export const UsersIcon = (p: IconProps) => (
  <Svg {...p}>
    <circle cx="7.5" cy="7" r="2.8" />
    <path d="M2.5 16c.7-2.5 2.6-3.9 5-3.9s4.3 1.4 5 3.9" />
    <path d="M13 4.4a2.8 2.8 0 0 1 0 5.2M14.6 12.4c1.4.5 2.4 1.7 2.9 3.6" />
  </Svg>
);
export const ActivityIcon = (p: IconProps) => (
  <Svg {...p}>
    <path d="M2.5 10h3l2-5 5 10 2-5h3" />
  </Svg>
);
export const ChartIcon = (p: IconProps) => (
  <Svg {...p}>
    <path d="M3.5 3.5v13h13" />
    <path d="M7 13V9.5M10.5 13V6.5M14 13v-5" />
  </Svg>
);
export const MailIcon = (p: IconProps) => (
  <Svg {...p}>
    <rect x="2.5" y="4.5" width="15" height="11" rx="1.4" />
    <path d="m3 5.5 7 5.5 7-5.5" />
  </Svg>
);
export const KeyIcon = (p: IconProps) => (
  <Svg {...p}>
    <circle cx="6.8" cy="13.2" r="3.3" />
    <path d="m9.2 10.8 7.3-7.3M13.6 6.4l2.1 2.1M11.6 8.4l1.6 1.6" />
  </Svg>
);
export const LockIcon = (p: IconProps) => (
  <Svg {...p}>
    <rect x="4" y="8.5" width="12" height="8.5" rx="1.4" />
    <path d="M6.8 8.5V6.3a3.2 3.2 0 0 1 6.4 0v2.2" />
  </Svg>
);
export const SmartphoneCodeIcon = (p: IconProps) => (
  <Svg {...p}>
    <rect x="5" y="2.5" width="10" height="15" rx="1.8" />
    <path d="M7.9 10h.01M10 10h.01M12.1 10h.01" strokeWidth={2.4} />
    <path d="M8.8 15h2.4" />
  </Svg>
);
export const ListIcon = (p: IconProps) => (
  <Svg {...p}>
    <path d="M7.5 5.5h9M7.5 10h9M7.5 14.5h9M3.5 5.5h.01M3.5 10h.01M3.5 14.5h.01" />
  </Svg>
);
export const ChevronRightIcon = (p: IconProps) => (
  <Svg {...p}>
    <path d="m8 5 5 5-5 5" />
  </Svg>
);
export const ArrowLeftIcon = (p: IconProps) => (
  <Svg {...p}>
    <path d="M16 10H4.5M9 5 4 10l5 5" />
  </Svg>
);
export const CheckIcon = (p: IconProps) => (
  <Svg {...p}>
    <path d="m4.5 10.5 3.5 3.5 7.5-8" />
  </Svg>
);
export const CheckCircleIcon = (p: IconProps) => (
  <Svg {...p}>
    <circle cx="10" cy="10" r="7.5" />
    <path d="m7 10.2 2.1 2.1L13.2 8" />
  </Svg>
);
export const AlertIcon = (p: IconProps) => (
  <Svg {...p}>
    <circle cx="10" cy="10" r="7.5" />
    <path d="M10 6.3v4.4M10 13.6h.01" />
  </Svg>
);
export const WarningIcon = (p: IconProps) => (
  <Svg {...p}>
    <path d="M9.1 3.6a1 1 0 0 1 1.8 0l6.4 11.6a1 1 0 0 1-.9 1.5H3.6a1 1 0 0 1-.9-1.5Z" />
    <path d="M10 8v3.6M10 14h.01" />
  </Svg>
);
export const InfoIcon = (p: IconProps) => (
  <Svg {...p}>
    <circle cx="10" cy="10" r="7.5" />
    <path d="M10 9v4.6M10 6.4h.01" />
  </Svg>
);
export const EyeIcon = (p: IconProps) => (
  <Svg {...p}>
    <path d="M1.8 10S4.7 4.5 10 4.5 18.2 10 18.2 10 15.3 15.5 10 15.5 1.8 10 1.8 10Z" />
    <circle cx="10" cy="10" r="2.5" />
  </Svg>
);
export const EyeOffIcon = (p: IconProps) => (
  <Svg {...p}>
    <path d="M8.2 4.7c.6-.1 1.2-.2 1.8-.2 5.3 0 8.2 5.5 8.2 5.5a14 14 0 0 1-2.2 2.9M5.4 5.9A14 14 0 0 0 1.8 10s2.9 5.5 8.2 5.5c1.6 0 3-.5 4.1-1.1" />
    <path d="M8.2 8.2a2.5 2.5 0 0 0 3.6 3.6M2.5 2.5l15 15" />
  </Svg>
);
export const SearchIcon = (p: IconProps) => (
  <Svg {...p}>
    <circle cx="9" cy="9" r="5.5" />
    <path d="m13 13 4 4" />
  </Svg>
);
export const CopyIcon = (p: IconProps) => (
  <Svg {...p}>
    <rect x="7" y="7" width="9.5" height="9.5" rx="1.4" />
    <path d="M13 7V4.9c0-.8-.6-1.4-1.4-1.4H4.9c-.8 0-1.4.6-1.4 1.4v6.7c0 .8.6 1.4 1.4 1.4H7" />
  </Svg>
);
export const DownloadIcon = (p: IconProps) => (
  <Svg {...p}>
    <path d="M10 3v9.5M6 8.5l4 4 4-4M3.5 16.5h13" />
  </Svg>
);
export const RefreshIcon = (p: IconProps) => (
  <Svg {...p}>
    <path d="M16 4v4h-4" />
    <path d="M15.6 8A6 6 0 1 0 16 11.5" />
  </Svg>
);
export const TrashIcon = (p: IconProps) => (
  <Svg {...p}>
    <path d="M3.5 5.5h13M8 5.5V4a1 1 0 0 1 1-1h2a1 1 0 0 1 1 1v1.5M5 5.5l.7 10.1a1 1 0 0 0 1 .9h6.6a1 1 0 0 0 1-.9L15 5.5" />
  </Svg>
);
export const MenuIcon = (p: IconProps) => (
  <Svg {...p}>
    <path d="M3 5.5h14M3 10h14M3 14.5h14" />
  </Svg>
);
export const XIcon = (p: IconProps) => (
  <Svg {...p}>
    <path d="m5 5 10 10M15 5 5 15" />
  </Svg>
);
export const ClockIcon = (p: IconProps) => (
  <Svg {...p}>
    <circle cx="10" cy="10" r="7.5" />
    <path d="M10 6v4.2l2.6 1.6" />
  </Svg>
);
export const GlobeIcon = (p: IconProps) => (
  <Svg {...p}>
    <circle cx="10" cy="10" r="7.5" />
    <path d="M2.5 10h15M10 2.5c2 2.1 3 4.6 3 7.5s-1 5.4-3 7.5c-2-2.1-3-4.6-3-7.5s1-5.4 3-7.5Z" />
  </Svg>
);
export const AppIcon = (p: IconProps) => (
  <Svg {...p}>
    <rect x="3" y="3" width="6" height="6" rx="1.2" />
    <rect x="11" y="3" width="6" height="6" rx="1.2" />
    <rect x="3" y="11" width="6" height="6" rx="1.2" />
    <rect x="11" y="11" width="6" height="6" rx="1.2" />
  </Svg>
);
export const BanIcon = (p: IconProps) => (
  <Svg {...p}>
    <circle cx="10" cy="10" r="7.5" />
    <path d="m4.7 4.7 10.6 10.6" />
  </Svg>
);
export const ExternalIcon = (p: IconProps) => (
  <Svg {...p}>
    <path d="M11.5 3.5h5v5M16.5 3.5 9 11M14.5 11.5v4a1 1 0 0 1-1 1h-9a1 1 0 0 1-1-1v-9a1 1 0 0 1 1-1h4" />
  </Svg>
);

export function GoogleLogo({ size = 18 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 18 18" aria-hidden="true">
      <path fill="#4285F4" d="M17.64 9.2c0-.64-.06-1.25-.16-1.84H9v3.48h4.84a4.14 4.14 0 0 1-1.8 2.72v2.26h2.92c1.7-1.57 2.68-3.88 2.68-6.62Z" />
      <path fill="#34A853" d="M9 18c2.43 0 4.47-.8 5.96-2.18l-2.92-2.26c-.8.54-1.84.86-3.04.86-2.34 0-4.33-1.58-5.04-3.7H.94v2.33A9 9 0 0 0 9 18Z" />
      <path fill="#FBBC05" d="M3.96 10.72A5.4 5.4 0 0 1 3.68 9c0-.6.1-1.18.28-1.72V4.95H.94A9 9 0 0 0 0 9c0 1.45.35 2.83.94 4.05l3.02-2.33Z" />
      <path fill="#EA4335" d="M9 3.58c1.32 0 2.5.45 3.44 1.35l2.58-2.58C13.46.9 11.43 0 9 0A9 9 0 0 0 .94 4.95l3.02 2.33C4.67 5.16 6.66 3.58 9 3.58Z" />
    </svg>
  );
}

export function GitHubLogo({ size = 18 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 16 16" aria-hidden="true" fill="#1c1917">
      <path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.01 8.01 0 0 0 16 8c0-4.42-3.58-8-8-8Z" />
    </svg>
  );
}
