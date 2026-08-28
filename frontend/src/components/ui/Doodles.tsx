import { useMemo } from "react";

interface DoodleItem {
  type: string;
  subType: number;
  width: number;
  height: number;
  style: React.CSSProperties;
}

const DOODLE_TYPES = [
  "crosshair",
  "plus",
  "bracket",
  "square",
  "dot",
  "wave",
  "triangle",
];

function generateDoodles(count: number): DoodleItem[] {
  const list: DoodleItem[] = [];
  for (let i = 0; i < count; i++) {
    const type = DOODLE_TYPES[Math.floor(Math.random() * DOODLE_TYPES.length)];
    const size = Math.floor(Math.random() * 20) + 12;
    const x = Math.random() * 100;
    const y = Math.random() * 100;
    const rotation = Math.floor(Math.random() * 4) * 90;
    const opacity = (Math.random() * 0.5 + 0.3).toFixed(2);

    list.push({
      type,
      subType: Math.floor(Math.random() * 4) + 1,
      width: size,
      height: size,
      style: {
        left: `${x}%`,
        top: `${y}%`,
        transform: `rotate(${rotation}deg)`,
        opacity,
        position: "absolute",
        color: "var(--text-color, #ffffff)",
      },
    });
  }
  return list;
}

function DoodleSvg({ type, subType }: { type: string; subType: number }) {
  const strokeWidth = "1.5";

  if (type === "crosshair") {
    return (
      <>
        <line x1="12" y1="0" x2="12" y2="24" strokeWidth={strokeWidth} />
        <line x1="0" y1="12" x2="24" y2="12" strokeWidth={strokeWidth} />
      </>
    );
  }
  if (type === "plus") {
    return <path d="M12 4v16M4 12h16" strokeWidth="2.5" />;
  }
  if (type === "bracket") {
    if (subType === 1)
      return <polyline points="24 0 0 0 0 24" strokeWidth={strokeWidth} />;
    if (subType === 2)
      return <polyline points="0 24 24 24 24 0" strokeWidth={strokeWidth} />;
    if (subType === 3)
      return <polyline points="24 24 0 24 0 0" strokeWidth={strokeWidth} />;
    return <polyline points="0 0 24 0 24 24" strokeWidth={strokeWidth} />;
  }
  if (type === "square") {
    return (
      <rect
        x="4"
        y="4"
        width="16"
        height="16"
        strokeWidth={strokeWidth}
        strokeDasharray="3 3"
      />
    );
  }
  if (type === "dot") {
    return <circle cx="12" cy="12" r="2" fill="currentColor" stroke="none" />;
  }
  if (type === "wave") {
    return <path d="M0 12 Q6 6 12 12 T24 12" strokeWidth={strokeWidth} />;
  }
  if (type === "triangle") {
    return (
      <polygon points="12 4 20 20 4 20" strokeWidth="1" />
    );
  }
  return null;
}

export function Doodles({ count = 120 }: { count?: number }) {
  const doodleList = useMemo(() => generateDoodles(count), [count]);

  return (
    <div
      style={{
        position: "fixed",
        top: 0,
        left: 0,
        width: "100%",
        height: "100%",
        pointerEvents: "none",
        zIndex: 0,
        overflow: "hidden",
      }}
    >
      {doodleList.map((doodle, index) => (
        <svg
          key={index}
          style={doodle.style}
          width={doodle.width}
          height={doodle.height}
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
        >
          <DoodleSvg type={doodle.type} subType={doodle.subType} />
        </svg>
      ))}
    </div>
  );
}
