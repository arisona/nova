import * as React from 'react';

// Indicative pictures of the content modules, drawn in the colors of the selected
// palette. They are keyed by module name: keep them in sync with the names in
// `server/src/content`. Unknown modules show the palette as a soft gradient.

const WIDTH = 320;
const HEIGHT = 32;

// mulberry32: a small seeded generator, so every pictogram looks the same each time
const random = (seed: number) => {
  let a = seed;
  return () => {
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
};

const parseHex = (hex: string) =>
  [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16));

const mix = (a: string, b: string, t: number) => {
  const [ar, ag, ab] = parseHex(a);
  const [br, bg, bb] = parseHex(b);
  const channel = (x: number, y: number) => Math.round(x + (y - x) * t);
  return `rgb(${String(channel(ar, br))}, ${String(channel(ag, bg))}, ${String(channel(ab, bb))})`;
};

// The blend at palette position q, clamped to the first and last color, like the
// server's Heat picker for single-color modules.
const at = (colors: string[], q: number) => {
  const position = Math.min(Math.max(q, 0), colors.length - 1);
  const index = Math.min(Math.floor(position), colors.length - 1);
  const next = Math.min(index + 1, colors.length - 1);
  return mix(colors[index], colors[next], position - index);
};

const cycle = (colors: string[], i: number) => colors[i % colors.length];

interface Drawing {
  colors: string[];
  heat: number;
  id: string;
}

// Orion's Belt: soft noise blobs blending into each other.
const Flux = ({ colors, id }: Drawing) => {
  const rand = random(1);
  return (
    <>
      <filter id={`${id}-blur`} x="-20%" y="-50%" width="140%" height="200%">
        <feGaussianBlur stdDeviation="5" />
      </filter>
      <g filter={`url(#${id}-blur)`}>
        {Array.from({ length: 12 }, (_, i) => (
          <ellipse
            key={i}
            cx={i * 28 + rand() * 12}
            cy={6 + rand() * 20}
            rx={14 + rand() * 12}
            ry={6 + rand() * 6}
            fill={cycle(colors, i)}
          />
        ))}
      </g>
    </>
  );
};

// Oort Cloud: right-angled trails fading behind bright heads.
const LightCycles = ({ colors, id }: Drawing) => {
  const rand = random(2);
  const lanes = [6, 13, 20, 27];
  return (
    <>
      {[0, 1, 2, 3].map((trail) => {
        let x = rand() * 30;
        let y = lanes[Math.floor(rand() * lanes.length)];
        const start = x;
        const points = [`${String(x)},${String(y)}`];
        while (x < 230 + trail * 20) {
          x += 20 + rand() * 40;
          points.push(`${String(x)},${String(y)}`);
          y = lanes[Math.floor(rand() * lanes.length)];
          points.push(`${String(x)},${String(y)}`);
        }
        const color = cycle(colors, trail);
        const gradient = `${id}-trail${String(trail)}`;
        return (
          <React.Fragment key={trail}>
            <linearGradient
              id={gradient}
              gradientUnits="userSpaceOnUse"
              x1={start}
              x2={x}
              y1={0}
              y2={0}
            >
              <stop offset="0" stopColor={color} stopOpacity={0} />
              <stop offset="1" stopColor={color} stopOpacity={1} />
            </linearGradient>
            <polyline
              points={points.join(' ')}
              fill="none"
              stroke={`url(#${gradient})`}
              strokeWidth={2.5}
              strokeLinejoin="miter"
            />
            <rect
              x={x - 2.5}
              y={y - 2.5}
              width={5}
              height={5}
              fill={mix(colors[trail % colors.length], '#ffffff', 0.4)}
            />
          </React.Fragment>
        );
      })}
    </>
  );
};

// Andromeda: soft orbs melting into each other, plus two small bouncing balls.
const Melange = ({ colors, id }: Drawing) => {
  const rand = random(3);
  return (
    <>
      {colors.map((color, i) => (
        <radialGradient key={i} id={`${id}-orb${String(i)}`}>
          <stop offset="0" stopColor={color} stopOpacity={1} />
          <stop offset="0.6" stopColor={color} stopOpacity={0.8} />
          <stop offset="1" stopColor={color} stopOpacity={0} />
        </radialGradient>
      ))}
      {Array.from({ length: 11 }, (_, i) => (
        <circle
          key={i}
          cx={i * 30 + rand() * 14}
          cy={8 + rand() * 16}
          r={8 + rand() * 10}
          fill={`url(#${id}-orb${String(i % colors.length)})`}
        />
      ))}
      <circle cx={250} cy={6} r={3} fill={cycle(colors, 1)} />
      <circle cx={95} cy={27} r={3} fill={cycle(colors, 2)} />
    </>
  );
};

// Tannhäuser Gate: flame tongues in the color Heat picks, sparks and a star glint.
const Flames = ({ colors, heat, id }: Drawing) => {
  const rand = random(4);
  const hot = heat * (colors.length - 1);
  const flame = at(colors, hot);
  const cool = at(colors, hot - 0.5);
  return (
    <>
      <linearGradient id={`${id}-flame`} x1="0" y1="1" x2="0" y2="0">
        <stop offset="0" stopColor={flame} stopOpacity={1} />
        <stop offset="0.6" stopColor={flame} stopOpacity={0.75} />
        <stop offset="1" stopColor={cool} stopOpacity={0.15} />
      </linearGradient>
      <rect x={0} y={28} width={WIDTH} height={4} fill={flame} opacity={0.6} />
      {Array.from({ length: 16 }, (_, i) => {
        const x = i * 21 - 6 + rand() * 6;
        const w = 16 + rand() * 12;
        const top = HEIGHT - 10 - rand() * 20;
        const middle = (HEIGHT + top) / 2;
        return (
          <path
            key={i}
            d={`M ${String(x)} 32 Q ${String(x + w * 0.05)} ${String(middle)} ${String(x + w * 0.5)} ${String(top)} Q ${String(x + w * 0.95)} ${String(middle)} ${String(x + w)} 32 Z`}
            fill={`url(#${id}-flame)`}
          />
        );
      })}
      {Array.from({ length: 14 }, (_, i) => (
        <circle
          key={i}
          cx={rand() * WIDTH}
          cy={2 + rand() * 12}
          r={0.8 + rand()}
          fill={flame}
        />
      ))}
      <g stroke={flame} strokeWidth={1} strokeLinecap="round">
        <line x1={204} y1={7} x2={216} y2={7} />
        <line x1={210} y1={1} x2={210} y2={13} />
      </g>
    </>
  );
};

// Betelgeuse: a wavy water level in the color Heat picks, raindrops and ripples.
const Water = ({ colors, heat, id }: Drawing) => {
  const rand = random(5);
  const hot = heat * (colors.length - 1);
  const water = at(colors, hot);
  const deep = at(colors, hot - 0.8);
  const drop = at(colors, hot + 0.6);
  const surface = (x: number) =>
    17 + 3 * Math.sin(x / 19) + 1.5 * Math.sin(x / 7 + 1);
  const points = Array.from({ length: 81 }, (_, i) => {
    const x = (i * WIDTH) / 80;
    return `${String(x)},${surface(x).toFixed(1)}`;
  });
  return (
    <>
      <linearGradient id={`${id}-water`} x1="0" y1="0" x2="0" y2="1">
        <stop offset="0" stopColor={water} />
        <stop offset="1" stopColor={deep} />
      </linearGradient>
      <polygon
        points={`0,32 ${points.join(' ')} ${String(WIDTH)},32`}
        fill={`url(#${id}-water)`}
      />
      <g stroke={drop} strokeWidth={1.2} strokeLinecap="round">
        {Array.from({ length: 14 }, (_, i) => {
          const x = i * 23 + rand() * 12;
          const y = 1 + rand() * 7;
          return (
            <line key={i} x1={x} y1={y} x2={x - 1} y2={y + 5} opacity={0.85} />
          );
        })}
      </g>
      {[70, 180, 265].map((x) => (
        <ellipse
          key={x}
          cx={x}
          cy={surface(x)}
          rx={9}
          ry={1.8}
          fill="none"
          stroke={drop}
          strokeWidth={0.8}
          opacity={0.7}
        />
      ))}
    </>
  );
};

// Pleiades: Turing spots and short stripes, colored by a slow drift through the palette.
const Shimmer = ({ colors, id }: Drawing) => {
  const rand = random(6);
  const spots: React.ReactNode[] = [];
  for (let row = 0; row < 4; row++) {
    for (let col = 0; col < 28; col++) {
      const x = col * 12 + (row % 2) * 6 + rand() * 3;
      const y = 4 + row * 8 + rand() * 2;
      const q = (col / 28) * Math.min(1.5, colors.length - 1) + rand() * 0.5;
      spots.push(
        <ellipse
          key={`${String(row)}-${String(col)}`}
          cx={x}
          cy={y}
          rx={2.5 + rand() * 3}
          ry={1.6 + rand() * 1.4}
          transform={`rotate(${String(rand() * 180)} ${String(x)} ${String(y)})`}
          fill={at(colors, q)}
        />
      );
    }
  }
  return (
    <>
      <filter id={`${id}-soft`}>
        <feGaussianBlur stdDeviation="0.7" />
      </filter>
      <g filter={`url(#${id}-soft)`}>{spots}</g>
    </>
  );
};

const Fallback = ({ colors, id }: Drawing) => (
  <>
    <linearGradient id={`${id}-palette`}>
      {colors.map((color, i) => (
        <stop
          key={i}
          offset={colors.length > 1 ? i / (colors.length - 1) : 0}
          stopColor={color}
        />
      ))}
    </linearGradient>
    <rect width={WIDTH} height={HEIGHT} fill={`url(#${id}-palette)`} />
  </>
);

const DRAWINGS: Record<string, (drawing: Drawing) => React.ReactNode> = {
  Andromeda: Melange,
  Betelgeuse: Water,
  'Oort Cloud': LightCycles,
  "Orion's Belt": Flux,
  Pleiades: Shimmer,
  'Tannhäuser Gate': Flames,
};

export const ContentPictogram = ({
  name,
  colors,
  heat,
}: {
  name: string;
  colors: string[];
  heat: number;
}) => {
  const id = React.useId().replace(/:/g, '');
  const Draw = DRAWINGS[name] ?? Fallback;
  const palette = colors.length > 0 ? colors : ['#808080'];
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
      <Draw colors={palette} heat={heat} id={id} />
    </svg>
  );
};
