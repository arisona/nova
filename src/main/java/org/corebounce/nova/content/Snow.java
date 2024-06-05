package org.corebounce.nova.content;

import org.corebounce.nova.Content;

public class Snow extends Content {

  private static final int N = 50;
  private final double[][] phase;
  private final double[][] speed;
  private final float[] flock = new float[N * 2];

  public Snow(int dimI, int dimJ, int dimK) {
    super("Snow", dimI, dimJ, dimK);
    phase = new double[dimI][dimJ];
    speed = new double[dimI][dimJ];
    for (int i = 0; i < dimI; i++) {
      for (int j = 0; j < dimJ; j++) {
        phase[i][j] = Math.random();
        speed[i][j] = Math.random();
      }
    }
    int m = 8;
    double m1 = m - 1;
    for (int i = 0; i < m; i++) {
      flock[i] = (float) (Math.sin((Math.PI * i) / m1) * Math.sin((Math.PI * i) / m1));
    }
  }

  @Override
  public void fillFrame(float[] rgbFrame, double timeInSec) {
    int dimK1 = dimK - 1;
    for (int k = 0; k < dimK; k++) {
      for (int i = 0; i < dimI; i++) {
        for (int j = 0; j < dimJ; j++) {
          int idx = ((int) ((phase[i][j] * N + timeInSec * (0.3 + speed[i][j])) * dimK) + k) % flock.length;
          float v = flock[idx];
          setVoxel(rgbFrame, i, j, k, v, v, v);
          if (k == dimK1 && idx == N) {
            phase[i][j] = Math.random();
            speed[i][j] = Math.random();
          }
        }
      }
    }
  }
}
