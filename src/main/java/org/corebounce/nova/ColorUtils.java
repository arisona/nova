package org.corebounce.nova;

public class ColorUtils {

    public static float[] rgbToHsv(float r, float g, float b) {
        float max = Math.max(Math.max(r, g), b);
        float min = Math.min(Math.min(r, g), b);
        float delta = max - min;

        float h = 0;
        if (delta != 0) {
            if (max == r) {
                h = (g - b) / delta;
            } else if (max == g) {
                h = 2 + (b - r) / delta;
            } else if (max == b) {
                h = 4 + (r - g) / delta;
            }
            h /= 6;
            if (h < 0) {
                h += 1;
            }
        }
        float s = max != 0 ? delta / max : 0;
        float v = max;
        return new float[]{h, s, v};
    }

    public static float[] hsvToRgb(float h, float s, float v) {
        if (s == 0) {
            return new float[]{v, v, v};
        }

        h *= 6;
        int i = (int) Math.floor(h);
        float f = h - i;
        float p = v * (1 - s);
        float q = v * (1 - s * f);
        float t = v * (1 - s * (1 - f));
        return switch (i) {
            case 0 -> new float[]{v, t, p};
            case 1 -> new float[]{q, v, p};
            case 2 -> new float[]{p, v, t};
            case 3 -> new float[]{p, q, v};
            case 4 -> new float[]{t, p, v};
            default -> new float[]{v, p, q};
        };
    }
}
