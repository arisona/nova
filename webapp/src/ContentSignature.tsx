import * as React from 'react';

// The content module's spectral signature: lines like a star's emission spectrum, placed
// by a hash of the module's name and drawn in the selected palette's colors, so every
// module, including new ones, gets its own.

const WIDTH = 320;
const HEIGHT = 32;

// FNV-1a over the name's UTF-16 code units, the seed for its lines.
const hash = (text: string) => {
  let h = 0x811c9dc5;
  for (let i = 0; i < text.length; i++) {
    h ^= text.charCodeAt(i);
    h = Math.imul(h, 0x01000193);
  }
  return h >>> 0;
};

// mulberry32: a small seeded generator, so a signature looks the same each time.
const random = (seed: number) => {
  let a = seed;
  return () => {
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
};

interface Line {
  x: number;
  width: number;
  brightness: number;
  color: string;
}

const spectrum = (name: string, colors: string[]): Line[] => {
  const next = random(hash(name));
  const lines: Omit<Line, 'color'>[] = [];
  // A series of lines converging towards a limit, like the hydrogen series...
  let x = 20 + next() * 120;
  let step = 22 + next() * 30;
  const ratio = 0.62 + next() * 0.2;
  while (step > 3 && x < WIDTH - 4) {
    lines.push({
      x,
      width: 1.2 + next() * 2.2,
      brightness: 0.55 + next() * 0.45,
    });
    x += step;
    step *= ratio;
  }
  // ...and loose lines.
  const loose = 4 + Math.floor(next() * 6);
  for (let i = 0; i < loose; i++) {
    lines.push({
      x: 4 + next() * (WIDTH - 8),
      width: 0.8 + next() * 3.2,
      brightness: 0.35 + next() * 0.65,
    });
  }
  return lines.map((line) => ({
    ...line,
    color: colors[Math.floor(next() * colors.length)],
  }));
};

export const ContentSignature = ({
  name,
  colors,
}: {
  name: string;
  colors: string[];
}) => {
  const lines = spectrum(name, colors.length > 0 ? colors : ['#808080']);
  return (
    <svg
      aria-hidden
      viewBox={`0 0 ${String(WIDTH)} ${String(HEIGHT)}`}
      preserveAspectRatio="xMidYMid slice"
      style={{
        width: '100%',
        height: '100%',
        display: 'block',
        background: '#000',
      }}
    >
      {lines.map(({ x, width, brightness, color }, index) => (
        <React.Fragment key={index}>
          {/* A faint glow around each line. */}
          <rect
            x={x - 2.5 * width}
            width={5 * width}
            height={HEIGHT}
            fill={color}
            opacity={0.18 * brightness}
          />
          <rect
            x={x - width / 2}
            width={width}
            height={HEIGHT}
            fill={color}
            opacity={brightness}
          />
        </React.Fragment>
      ))}
    </svg>
  );
};
