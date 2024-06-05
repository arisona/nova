package org.corebounce.nova.content;

import org.corebounce.nova.Content;

public class Cylinder extends Content {

  private final double dimI1;
  private final double dimJ1;
  private final float[][][] buffer;

  public Cylinder(int dimI, int dimJ, int dimK) {
    super("Cylinder", dimI, dimJ, dimK);
    dimI1 = dimI / 2.0;
    dimJ1 = dimJ / 2.0;
    buffer = new float[dimI][dimJ][dimK];
  }

  @Override
  public void fillFrame(float[] rgbFrame, double timeInSec) {
    float speed = getSpeed();
    if (speed < 0.1f) {
      speed = 0.1f;
    }
    if (speed > 1f) {
      speed = 1f;
    }
    timeInSec *= 0.2;
    double twist = 0.1;
    final float dimFactor = ((1f - speed) * 0.3f) + 0.7f;
    for (int k = 0; k < dimK; k++) {
      final double t = (timeInSec + k * twist) * Math.PI;
      double sin = Math.sin(t);
      double cos = Math.cos(t);
      int x = (int) (dimI1 + (sin * dimI1));
      int y = (int) (dimJ1 + (cos * dimJ1));
      int x0 = (int) (dimI1 + (sin * (dimI1 - 1)));
      int y0 = (int) (dimJ1 + (cos * (dimJ1 - 1)));
      for (int i = 0; i < dimI; i++) {
        for (int j = 0; j < dimJ; j++) {
          if ((x == i && y == j) || (x0 == i && y0 == j)) {
            buffer[i][j][k] = 1f;
          } else {
            buffer[i][j][k] *= dimFactor;
          }
          final float b = buffer[i][j][k];
          setVoxel(rgbFrame, i, j, k, b, b, b);
        }
      }
    }
  }
}
