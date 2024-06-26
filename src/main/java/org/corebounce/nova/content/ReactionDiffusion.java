package org.corebounce.nova.content;

import java.util.Random;
import org.corebounce.nova.ColorUtils;
import org.corebounce.nova.Content;

public class ReactionDiffusion extends Content {

  public int nx;
  public int ny;
  public int nz;
  int nYZ;

  // float CA = 2.6;
  // float CB = 24;
  //	float CA = 0.7f;
  //	float CB = 14f;
  float CA;
  float CB;

  // boolean endless;
  // boolean drawBox;
  float high, low;
  // float iso = 0.5f;
  // float border = 0f;

  float[] A;
  float[] B;
  float[] An;
  float[] Bn;
  int[] inbs = new int[6];

  float[][] settings;
  public int iSetting;
  public static final int PATTERN_NULL = 0;
  public static final int PATTERN_CHEETAH = 1;
  public static final int PATTERN_COLONY = 2;
  public static final int PATTERN_FINE = 3;
  public static final int PATTERN_FINGERPRINT = 4;
  public static final int PATTERN_MAZE = 5;
  public static final int PATTERN_POCKED = 6;
  public static final int PATTERN_TEST = 7;
  public Random random;

  public ReactionDiffusion(int dimI, int dimJ, int dimK) {
    super("ReactionDiffusion", dimI, dimJ, dimK);
    this.nx = dimI;
    this.ny = dimJ;
    this.nz = dimK;
    nYZ = ny * nz;
    iSetting = 1;
    setupReaction();
  }

  int a = 0;
  int b = 1;
  int c = 0;

  @Override
  public void fillFrame(float[] rgbFrame, double timeInSec) {
    int i = 0;
    // diffuse;
    //		if ((int)(timeInSec/4)!=a) {
    //			iSetting = (iSetting+1)%5;
    //			a = (int)(timeInSec/4);
    //		}
    //		CA = settings[iSetting][0];
    //		CB = settings[iSetting][1];
    double fct = (Math.sin(timeInSec) * 0.5) + 0.5;
    CA = (float) (fct * settings[a][0] + (1 - fct) * settings[b][0]);
    CB = (float) (fct * settings[a][1] + (1 - fct) * settings[b][1]);
    if ((int) (timeInSec / 35) != c) {
      a = b;
      b = (b + 1) % 8;
      c = (int) (timeInSec / 35);
      //			System.out.println(a+" : "+ b);
    }

    for (int x = 0; x < nx; x++) {
      for (int y = 0; y < ny; y++) {
        for (int z = 0; z < nz; z++) {
          float dA = 0.01f * CA * A[i];
          float dB = 0.01f * CB * B[i];
          if (x > 0) {
            inbs[0] = getIndex(x - 1, y, z);
          } else {
            inbs[0] = getIndex(nx - 1, y, z);
          }
          if (x < nx - 1) {
            inbs[1] = getIndex(x + 1, y, z);
          } else {
            inbs[1] = getIndex(0, y, z);
          }
          if (y > 0) {
            inbs[2] = getIndex(x, y - 1, z);
          } else {
            inbs[2] = getIndex(x, ny - 1, z);
          }
          if (y < ny - 1) {
            inbs[3] = getIndex(x, y + 1, z);
          } else {
            inbs[3] = getIndex(x, 0, z);
          }
          if (z > 0) {
            inbs[4] = getIndex(x, y, z - 1);
          } else {
            inbs[4] = getIndex(x, y, nz - 1);
          }
          if (z < nz - 1) {
            inbs[5] = getIndex(x, y, z + 1);
          } else {
            inbs[5] = getIndex(x, y, 0);
          }
          int nNbs = 0;
          for (int j = 0; j < inbs.length; j++) {
            if (inbs[j] >= 0) {
              nNbs++;
              An[inbs[j]] += dA;
              Bn[inbs[j]] += dB;
            }
          }
          An[i] -= nNbs * dA;
          Bn[i] -= nNbs * dB;
          i++;
        }
      }
    }

    // react
    for (i = 0; i < An.length; i++) {
      An[i] += 0.01f * (A[i] * B[i] - A[i] - 12.0f);
      Bn[i] += 0.01f * (16.0f - A[i] * B[i]);
    }

    // swap
    for (i = 0; i < An.length; i++) {
      A[i] += An[i];
      if (A[i] < 0.0) {
        A[i] = 0.0f;
      }
      B[i] += Bn[i];
      if (B[i] < 0.0) {
        B[i] = 0.0f;
      }
      An[i] = 0;
      Bn[i] = 0;
    }

    // max min
    low = Float.MAX_VALUE;
    high = Float.NEGATIVE_INFINITY;
    for (i = 0; i < An.length; i++) {
      high = Math.max(A[i], high);
      low = Math.min(A[i], low);
    }

    // render
    for (int x = 0; x < nx; x++) {
      for (int y = 0; y < ny; y++) {
        for (int z = 0; z < nz; z++) {
          int ix = getIndex(x, y, z);
          float f = (A[ix] - low) / (high - low);
          float[] rgb = ColorUtils.hsvToRgb(f, 1, 1 - f);
          //					setVoxel(rgbFrame, x, y, z, 0, f*0.5f, f);
          setVoxel(rgbFrame, x, y, z, rgb[0], rgb[1], rgb[2]);
        }
      }
    }
  }

  public void setupReaction() {
    settings = new float[8][2];
    settings[0] = new float[] { 0.7f, 0.14f };
    settings[1] = new float[] { 3.5f, 16 }; // 3.5d, 16d CHEETAH
    settings[2] = new float[] { 1.6f, 6f }; // 1.6d, 6d COLONY
    settings[3] = new float[] { 0.1f, 1f }; // 0.1d, 1d FINE
    settings[4] = new float[] { 1f, 16f }; // 1d, 16d FINGERPRINT
    settings[5] = new float[] { 0.7f, 0.14f }; //{ 2.6f, 24f };// 2.6d, 24d MAZE
    settings[6] = new float[] { 1, 3 }; // 1d, 3d POCKED
    settings[7] = new float[] { 1, 14 }; // 1d, 3d POCKED
    CA = settings[iSetting][0];
    CB = settings[iSetting][1];

    A = new float[nx * ny * nz];
    B = new float[nx * ny * nz];
    An = new float[nx * ny * nz];
    Bn = new float[nx * ny * nz];
    //		initRandom();
    initSym();
  }

  public int getIndex(int x, int y, int z) {
    return x * nYZ + y * nz + z;
    //		return (z + nz * (x + y * nx));
    //		(k + (dimK * (i + j * dimI)))
  }

  public void copyValues(float[] values) {
    System.arraycopy(A, 0, values, 0, A.length);
  }

  public void initRandom() {
    if (random != null) {
      for (int i = 0; i < An.length; i++) {
        A[i] = -7 + (float) random.nextFloat() * (17 + 7);
        B[i] = -7 + (float) random.nextFloat() * (17 + 7);
        An[i] = Bn[i] = 0;
      }
    } else {
      for (int i = 0; i < An.length; i++) {
        A[i] = -7 + (float) Math.random() * (17 + 7);
        B[i] = -7 + (float) Math.random() * (17 + 7);
        An[i] = Bn[i] = 0;
      }
    }
  }

  public void initSym() {
    for (int i = 0; i < An.length; i++) {
      A[i] = -7;
      B[i] = -7;
      An[i] = Bn[i] = 0;
    }
    for (int x = nx / 2 - 2; x < nx / 2 + 2; x++) {
      for (int y = ny / 2 - 2; y < ny / 2 + 2; y++) {
        for (int z = nz / 2 - 2; z < nz / 2 + 2; z++) {
          int ix = getIndex(x, y, z);
          A[ix] = 17;
          B[ix] = 17;
        }
      }
    }
  }
}
