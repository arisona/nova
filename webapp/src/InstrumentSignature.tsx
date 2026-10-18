import * as React from 'react';

import { hash, random } from './seeded';

// The instrument's signature: a glowing trace like an oscillogram, the time-domain
// counterpart of the content's spectrum. A few partials under an envelope of swells and
// hits, all seeded by a hash of the instrument's name and drawn in the selected palette's
// colors, so every instrument, including new ones, gets its own.

const WIDTH = 320;
const HEIGHT = 32;
const SAMPLES = 2 * WIDTH;

interface Partial {
  ratio: number;
  amplitude: number;
  phase: number;
}

interface Hit {
  x: number;
  attack: number;
  decay: number;
  peak: number;
}

const trace = (name: string): string => {
  const next = random(hash(name));
  // The fundamental's cycles across the strip, and its partials: mostly harmonic, some
  // slightly detuned so they beat.
  const cycles = 10 + next() * 18;
  const partials: Partial[] = [{ ratio: 1, amplitude: 1, phase: 0 }];
  const count = 1 + Math.floor(next() * 3);
  for (let i = 0; i < count; i++) {
    const harmonic = 2 + Math.floor(next() * 5);
    partials.push({
      ratio: harmonic * (1 + (next() < 0.4 ? next() * 0.04 : 0)),
      amplitude: (0.3 + 0.7 * next()) / harmonic,
      phase: next() * 2 * Math.PI,
    });
  }
  const total = partials.reduce((sum, { amplitude }) => sum + amplitude, 0);
  // The envelope: what sustains, and the hits rising out of it.
  const sustain = 0.08 + 0.3 * next();
  const hits: Hit[] = [];
  const hitCount = 2 + Math.floor(next() * 4);
  for (let i = 0; i < hitCount; i++) {
    hits.push({
      x: next() * WIDTH,
      attack: 2 + next() * 30,
      decay: 10 + next() * 120,
      peak: 0.55 + 0.45 * next(),
    });
  }
  const envelope = (x: number) =>
    hits.reduce((level, { x: start, attack, decay, peak }) => {
      const t = x - start;
      const shape =
        t < -attack ? 0 : t < 0 ? 1 + t / attack : Math.exp(-t / decay);
      return Math.max(level, peak * shape);
    }, sustain);

  const points: string[] = [];
  for (let i = 0; i <= SAMPLES; i++) {
    const x = (i / SAMPLES) * WIDTH;
    const phase = (x / WIDTH) * cycles * 2 * Math.PI;
    const wave = partials.reduce(
      (sum, { ratio, amplitude, phase: offset }) =>
        sum + amplitude * Math.sin(ratio * phase + offset),
      0
    );
    const y = HEIGHT / 2 - 0.4 * HEIGHT * envelope(x) * (wave / total);
    points.push(`${x.toFixed(2)},${y.toFixed(2)}`);
  }
  return `M${points.join('L')}`;
};

export const InstrumentSignature = ({
  name,
  colors,
}: {
  name: string;
  colors: string[];
}) => {
  const gradient = React.useId();
  const stops = colors.length > 0 ? colors : ['#808080'];
  const path = trace(name);
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
      <defs>
        {/* The palette's colors from left to right. */}
        <linearGradient
          id={gradient}
          gradientUnits="userSpaceOnUse"
          x1={0}
          x2={WIDTH}
          y1={0}
          y2={0}
        >
          {stops.map((color, index) => (
            <stop
              key={index}
              offset={stops.length > 1 ? index / (stops.length - 1) : 0}
              stopColor={color}
            />
          ))}
        </linearGradient>
      </defs>
      {/* A faint glow around the trace. */}
      <path
        d={path}
        fill="none"
        stroke={`url(#${gradient})`}
        strokeWidth={4}
        strokeLinejoin="round"
        opacity={0.25}
      />
      <path
        d={path}
        fill="none"
        stroke={`url(#${gradient})`}
        strokeWidth={1.4}
        strokeLinejoin="round"
      />
    </svg>
  );
};
