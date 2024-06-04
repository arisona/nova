package org.corebounce.nova.content;

public class Ball {

    public MVector pos, vel;
    public MVector minc, maxc;

    public Ball(double x, double y, double z) {
        pos = new MVector(x, y, z);
        vel = MVector.random3D();
        vel.y *= 3;
    }

    public void update(double t) {
        pos.add(MVector.mult(vel, t));
        if (pos.x > maxc.x || pos.x < minc.x) {
            vel.x *= -1;
        }
        if (pos.y > maxc.y || pos.y < minc.y) {
            vel.y *= -1.01;
			// vel.x = vel.x + rand(-chg,chg);
			// vel.z = vel.z + rand(-chg,chg);
        }
        if (pos.z > maxc.z || pos.z < minc.z) {
            vel.z *= -1;
        }
    }

    // private static double rand(double from, double to) {
    //     return from + (Math.random() * (to - from));
    // }
}
