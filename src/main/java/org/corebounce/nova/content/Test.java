package org.corebounce.nova.content;

import org.corebounce.nova.Content;

public class Test extends Content {

    public Test(int dimI, int dimJ, int dimK) {
        super("Test", dimI, dimJ, dimK);
    }

    @Override
    public void fillFrame(float[] rgbFrame, double timeInSec) {
        float speed = 1;
        for (int i = 0; i < dimI; i++) {
            float v = (int) (timeInSec / speed % dimI) == i ? 1f : 0f;
            for (int k = 0; k < dimK; k++) {
                for (int j = 0; j < dimJ; j++) {
                    setVoxel(rgbFrame, i, j, k, v, v, v);
                }
            }
        }
    }
}
