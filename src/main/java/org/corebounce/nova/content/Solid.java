package org.corebounce.nova.content;

import java.util.Arrays;
import org.corebounce.nova.Content;

public class Solid extends Content {

  public Solid(int dimI, int dimJ, int dimK) {
    super("Solid", dimI, dimJ, dimK);
  }

  @Override
  public void fillFrame(float[] rgbFrame, double timeInSec) {
    Arrays.fill(rgbFrame, 1f);
  }
}
