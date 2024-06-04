package org.corebounce.nova;

import java.io.IOException;
import java.net.URISyntaxException;
import java.nio.file.FileSystemNotFoundException;
import java.nio.file.FileSystems;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Collections;
import java.util.Comparator;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import java.util.stream.Stream;

import org.corebounce.nova.content.Test;
import org.corebounce.util.Log;

/**
 * Content base class for NOVA Server content.
 */
public abstract class Content {

    /* name of content. */
    protected final String name;
    /* X-dimension in the horizontal plane of the NOVA hardware, always a multiple of 5. */
    protected final int dimI;
    /* Y-dimension in the horizontal plane of the NOVA hardware, always a multiple of 5. */
    protected final int dimJ;
    /* Z-dimension (vertical) of the NOVA hardware, always 10. */
    protected final int dimK;

    /**
     * Creates a content instance.
     *
     * @param name The name of the content.
     * @param dimI The X-dimension.
     * @param dimJ The Y-dimension.
     * @param dimK The Z-dimension.
     */
    protected Content(String name, int dimI, int dimJ, int dimK) {
        this.name = name;
        this.dimI = dimI;
        this.dimJ = dimJ;
        this.dimK = dimK;
    }

    /**
     * Utility function to set a RGB values of a voxel at position (i,j,k).
     *
     * @param rgbFrame The voxel frame to operate on.
     * @param i The X-position.
     * @param j The Y-position.
     * @param k The Z-position.
     * @param r The red value.
     * @param g The green value.
     * @param b The blue value.
     */
    protected void setVoxel(float[] rgbFrame, int i, int j, int k, float r, float g, float b) {
        int idx = 3 * (k + (dimK * (i + j * dimI)));
        rgbFrame[idx + 0] = r;
        rgbFrame[idx + 1] = g;
        rgbFrame[idx + 2] = b;
    }

    /**
     * Utility function to add RGB values of a voxel at position (i,j,k) to
     * rgbFrame.
     *
     * @param rgbFrame The voxel frame to operate on.
     * @param i The X-position.
     * @param j The Y-position.
     * @param k The Z-position.
     * @param r The red value.
     * @param g The green value.
     * @param b The blue value.
     */
    protected void addVoxel(float[] rgbFrame, int i, int j, int k, float r, float g, float b) {
        int idx = 3 * (k + (dimK * (i + j * dimI)));
        rgbFrame[idx + 0] += r;
        rgbFrame[idx + 1] += g;
        rgbFrame[idx + 2] += b;
    }

    /**
     * Utility function to add RGB values of a voxel at position (i,j,k) to
     * rgbFrame. Clamps to [0..1]
     *
     * @param rgbFrame The voxel frame to operate on.
     * @param i The X-position.
     * @param j The Y-position.
     * @param k The Z-position.
     * @param r The red value.
     * @param g The green value.
     * @param b The blue value.
     */
    protected void addVoxelClamp(final float[] rgbFrame, final int i, final int j, final int k, final float r, final float g, final float b) {
        int idx = 3 * (k + (dimK * (i + j * dimI)));
        addAndClamp(rgbFrame, idx + 0, r);
        addAndClamp(rgbFrame, idx + 1, g);
        addAndClamp(rgbFrame, idx + 2, b);
    }

    private static void addAndClamp(final float[] rgbFrame, final int idx, final float value) {
        float v = rgbFrame[idx] + value;
        if (v < 0f) {
            rgbFrame[idx] = 0f;
        } else if (v > 1f) {
            rgbFrame[idx] = 1f;
        } else {
            rgbFrame[idx] = v;
        }
    }

    private static final float SCALE = 5f;

    /**
     * Utility function to set a weighted RGB values of a voxel at position
     * (i,j,k).
     *
     * @param rgbFrame The voxel frame to operate on.
     * @param i The X-position.
     * @param j The Y-position.
     * @param k The Z-position.
     * @param r The red value.
     * @param g The green value.
     * @param b The blue value.
     * @param w The weight.
     */
    protected void setVoxel(float[] rgbFrame, int i, int j, int k, float r, float g, float b, float w) {
        int idx = 3 * (k + (dimK * (i + j * dimI)));
        w *= SCALE;
        float w1 = 1 - w;
        rgbFrame[idx + 0] = r * w + w1 * rgbFrame[idx + 0];
        rgbFrame[idx + 1] = g * w + w1 * rgbFrame[idx + 1];
        rgbFrame[idx + 2] = b * w + w1 * rgbFrame[idx + 2];
    }

    /**
     * Utility function to set a weighted RGB values of a voxel at position
     * (i,j,k).
     *
     * @param rgbFrame The voxel frame to operate on.
     * @param i The X-position.
     * @param j The Y-position.
     * @param k The Z-position.
     * @param r The red value.
     * @param g The green value.
     * @param b The blue value.
     * @param wr The red weight.
     * @param wg The green weight.
     * @param wb The blue weight.
     */
    protected void setVoxel(float[] rgbFrame, int i, int j, int k, float r, float g, float b, float wr, float wg, float wb) {
        int idx = 3 * (k + (dimK * (i + j * dimI)));
        wr *= SCALE;
        wg *= SCALE;
        wb *= SCALE;
        rgbFrame[idx + 0] = r * wr + (1 - wr) * rgbFrame[idx + 0];
        rgbFrame[idx + 1] = g * wg + (1 - wg) * rgbFrame[idx + 1];
        rgbFrame[idx + 2] = b * wb + (1 - wb) * rgbFrame[idx + 2];
    }

    protected final float getSpeed() {
        return NOVAControl.get().getState().getSpeed();
    }

    /**
     * Called when a content is activated.
     */
    public void start() {
    }

    /**
     * Called when a content is deactivated.
     */
    public void stop() {
    }

    /**
     * Request a voxel frame to be filled. Must complete in less than 40ms in
     * order to keep up with the 25 Hz frame rate of the display.
     *
     * @param rgbFrame The voxel frame to operate on.
     * @param timeInSec The relative animation time starting from 0.
     */
    public abstract void fillFrame(float[] rgbFrame, double timeInSec);

    /**
     * Returns the name of the content.
     */
    @Override
    public String toString() {
        return name;
    }

    /**
     * Called by the server to get a list of content instances.
     *
     * @return The list of content instances. Usually just this instance.
     */
    public List<Content> getContents() {
        return Collections.singletonList(this);
    }

    static List<Content> createContent(State state) {
        var contents = new ArrayList<Content>();
        try {
            for (var cls : findAllContentClasses(Test.class.getPackageName())) {
                Content c = cls.getConstructor(int.class, int.class, int.class)
                        .newInstance(state.getDimI(), state.getDimJ(), state.getDimK());
                for (Content ci : c.getContents()) {
                    Log.info("Adding content '" + ci + "'");
                    contents.add(ci);
                }
            }
        } catch (Throwable t) {
            Log.warning(t, "Could not load content");
        }
        contents.sort(Comparator.comparing(Object::toString));
        return contents;
    }

    private static Set<Class<Content>> findAllContentClasses(String packageName) throws IOException, URISyntaxException {
        var packagePath = packageName.replace('.', '/');
        Path root;
        var pkg = ClassLoader.getSystemClassLoader().getResource(packagePath).toURI();
        if (pkg.toString().startsWith("jar:")) {
            try {
                root = FileSystems.getFileSystem(pkg).getPath(packagePath);
            } catch (FileSystemNotFoundException e) {
                root = FileSystems.newFileSystem(pkg, Collections.emptyMap()).getPath(packagePath);
            }
        } else {
            root = Paths.get(pkg);
        }

        var classes = new HashSet<Class<Content>>();
        try (Stream<Path> paths = Files.walk(root)) {
            paths.filter(Files::isRegularFile).forEach(file -> {
                try {
                    var path = file.toString().replace('/', '.');
                    var name = path.substring(path.indexOf(packageName), path.length() - ".class".length());
                    var cls = Class.forName(name);
                    if (cls.getSuperclass().equals(Content.class)) {
                        classes.add((Class<Content>) cls);
                    }
                } catch (ClassNotFoundException t) {
                }
            });
        }
        return classes;
    }
}
