package org.corebounce.nova.content;

import java.util.Arrays;

import org.corebounce.nova.Content;

public class Pulse extends Content {

    public Pulse(int dimI, int dimJ, int dimK) {
        super("Pulse", dimI, dimJ, dimK);
    }

    @Override
    public void fillFrame(float[] rgbFrame, double timeInSec) {
        float v = (float) Math.sin(timeInSec);
        Arrays.fill(rgbFrame, v * v);
    }

}
