package org.corebounce.nova;

import java.io.File;
import java.io.FileReader;
import java.io.FileWriter;
import java.io.IOException;
import java.net.DatagramSocket;
import java.net.Inet4Address;
import java.util.ArrayList;
import java.util.List;
import java.util.Properties;
import java.util.concurrent.LinkedBlockingQueue;
import java.util.concurrent.atomic.AtomicBoolean;

import org.corebounce.nova.content.Movie;
import org.corebounce.util.Log;
import org.jnetpcap.PcapException;

public final class NOVAControl implements IConstants {

    static final String PROPERTY_KEY_PORT = "port";
    static final String PROPERTY_KEY_INTERFACE = "nova";
    static final String PROPERTY_KEY_ADDRESS = "addr_";
    static final String PROPERTY_KEY_FLIP = "flip";
    static final String PROPERTY_KEY_CONTENT = "content";
    static final String PROPERTY_KEY_DURATION = "duration";
    static final String PROPERTY_KEY_MOVIES = "movies";
    static final String PROPERTY_KEY_CONFIG_DIR = "config_dir";

    private static final String CONTROL_PARAMS = "controlParams.properties";

    private static final int N_PACKET_BUFFERS = 1024;
    private static final int MODULE_QUEUE_SIZE = 4;
    private static final int FRAME_QUEUE_SIZE = MODULE_QUEUE_SIZE + 4;

    private static NOVAControl theControl;

    private final List<Content> availableContent = new ArrayList<>();
    private int selectedContent = 0;
    private float hue = 0.0f;
    private float saturation = 1.0f;
    private float brightness = 0.5f;
    private float speed = 0.0f;

    private final Properties properties = new Properties();
    private final EnetInterface device;
    private final NOVAConfig config;

    private final AtomicBoolean reset = new AtomicBoolean();

    private final byte[] selfIpAddr;
    private final int selfIpPort;
    private final byte[][] novaIpAddr = new byte[101][];
    private final LinkedBlockingQueue<int[][]> frameQ = new LinkedBlockingQueue<>();
    private final LinkedBlockingQueue<int[][]> txQ = new LinkedBlockingQueue<>();
    private final byte[][] packets = new byte[N_PACKET_BUFFERS][1100 + IConstants.UDP_PAYLOAD_OFF];
    private int packetPtr;

    private final Dispatcher dispatcher;

    private SyncGenerator syncGen;

    public NOVAControl(String configuration) throws IOException, InterruptedException, PcapException {
        if (theControl != null) {
            throw new RuntimeException("Cannot instantiate multiple NOVAControl instances.");
        }
        theControl = this;

        File propFile = new File(configuration).getAbsoluteFile();
        properties.load(new FileReader(propFile));

        int port = 80;
        try {
            port = Integer.parseInt(properties.getProperty(PROPERTY_KEY_PORT, "80"));
        } catch (Throwable t) {
        }
        Log.info("Listening on port " + port);

        device = EnetInterface.getInterface(properties.getProperty(PROPERTY_KEY_INTERFACE, "eth0"));
        Log.info("Using interface " + device.getName());
        device.open();

        config = new NOVAConfig(properties);

        availableContent.addAll(Content.createContent(config, properties));

        File movies = new File(properties.getProperty(PROPERTY_KEY_MOVIES, "."));
        if (movies.exists() && movies.isDirectory()) {
            Movie.ROOT_DIR = movies;
        }

        properties.put(PROPERTY_KEY_CONFIG_DIR, propFile.getParent());
        readControlParams();

        try (DatagramSocket socket = new DatagramSocket()) {
            selfIpAddr = Inet4Address.getLocalHost().getAddress();
            selfIpPort = socket.getLocalPort();
        }

        int maxModule = 0;
        for (int m : config.getModules()) {
            novaIpAddr[m] = new byte[]{(byte) NOVA_IP_0, (byte) NOVA_IP_1, (byte) NOVA_IP_2, (byte) m};
            maxModule = Math.max(maxModule, m);
        }

        for (int i = 0; i < FRAME_QUEUE_SIZE; i++) {
            frameQ.add(new int[maxModule + 1][config.dimI() * config.dimJ() * config.dimK() * 3]);
        }

        dispatcher = new Dispatcher(device, config);

        new UIServer(port);

        Thread thread = new Thread(this::streamTask, "Voxel Streamer");
        thread.setPriority(Thread.MIN_PRIORITY);
        thread.start();

        for (;;) {
            try {
                getStatus();
                Log.info("NOVA Status: " + config.numOperational() + " of " + config.numModules() + " operational");
                if (config.isOperational()) {
                    if (!(isOn())) {
                        novaOn();
                    }
                } else {
                    // note: not sure if we want this (if nova behaves unstable, just comment out)
                    if (isOn()) {
                        novaOff();
                        Log.info("NOVA Off: exiting");
                        System.exit(0);
                    }
                }
                Thread.sleep(isOn() ? 10000 : 1000);
            } catch (Throwable t) {
                Log.severe(t);
            }
        }
    }

    public static NOVAControl get() {
        return theControl;
    }

    public static void main(String[] args) throws IOException, InterruptedException, PcapException {
        if (args.length != 1) {
            System.out.println("Usage: " + NOVAControl.class.getName() + " <config_file>");
            System.exit(0);
        }

        Log.info("Using configuration: " + args[0]);

        new NOVAControl(args[0]);
    }

    List<Content> getAvailableContent() {
        return availableContent;
    }

    int getSelectedContent() {
        return selectedContent;
    }

    void setSelectedContent(int index) {
        try {
            availableContent.get(selectedContent).stop();
        } catch (Throwable t) {
        }
        selectedContent = index % availableContent.size();
        Log.info("setContent(" + selectedContent + "): " + availableContent.get(selectedContent));
        try {
            availableContent.get(selectedContent).start();
        } catch (Throwable t) {
        }
        writeControlParams();
    }

    public float getHue() {
        return hue;
    }

    public void setHue(float hue) {
        this.hue = hue;
    }

    public float getSaturation() {
        return saturation;
    }

    public void setSaturation(float saturation) {
        this.saturation = saturation;
    }

    public float getBrightness() {
        return brightness;
    }

    void setBrightness(float brightness) {
        this.brightness = brightness;
        writeControlParams();
    }

    public float getSpeed() {
        return speed;
    }

    void setSpeed(float speed) {
        this.speed = speed;
        writeControlParams();
    }

    boolean isOn() {
        return syncGen != null;
    }

    void novaOn() throws IOException, InterruptedException, PcapException {
        if (!(isOn()) && device != null) {
            Log.info("NOVA ON");
            reset();
            syncGen = new SyncGenerator(device, dispatcher);
            for (int i = -MODULE_QUEUE_SIZE; i < 0; i++) {
                sync(i);
            }
            syncGen.startSync();
            syncGen.setListener(this::sync);
        }
    }

    void novaOff() {
        if (isOn()) {
            Log.info("NOVA OFF");
            syncGen.setListener(null);
            syncGen.dispose();
            syncGen = null;
        }
    }

    void novaReset() {
        reset.set(true);
    }

    private void streamTask() {
        final float[] fframe = new float[config.dimI() * config.dimJ() * config.dimK() * 3];

        for (double time = 0;; time += 0.04 * Math.pow(2, speed)) {
            try {
                float[] rgb = ColorUtils.hsvToRgb(hue, saturation, brightness * brightness);
                float r = rgb[0] * 1023;
                float g = rgb[1] * 1023;
                float b = rgb[2] * 1023;

                int[][] frame = frameQ.take();
                boolean continueWithContent = availableContent.get(selectedContent).fillFrame(fframe, time);

                for (int m : config.getModules()) {
                    int off = config.getFrameOffset(m);
                    int idx = 0;

                    int[] pixels = frame[m];
                    if (config.flipK()) {
                        for (int i = config.moduleDimI(); i-- > 0;) {
                            for (int j = config.moduleDimJ(); j-- > 0;) {
                                for (int k = 0; k < config.moduleDimK(); k++) {
                                    int x = off + 3 * (j * config.dimI() * config.dimK() + i * config.dimK() + ((config.dimK() - 1) - k));
                                    float fr = fframe[x];
                                    float fg = fframe[x + 1];
                                    float fb = fframe[x + 2];
                                    pixels[idx++] = (((int) (r * fr * fr) << 20) & 0x3FF00000)
                                            | (((int) (g * fg * fg) << 10) & 0x000FFC00)
                                            | ((int) (b * fb * fb) & 0x000003FF);
                                }
                            }
                        }
                    } else {
                        for (int i = config.moduleDimI(); i-- > 0;) {
                            for (int j = config.moduleDimJ(); j-- > 0;) {
                                for (int k = 0; k < config.moduleDimK(); k++) {
                                    int x = off + 3 * (j * config.dimI() * config.dimK() + i * config.dimK() + k);
                                    float fr = fframe[x];
                                    float fg = fframe[x + 1];
                                    float fb = fframe[x + 2];
                                    pixels[idx++] = (((int) (r * fr * fr) << 20) & 0x3FF00000)
                                            | (((int) (g * fg * fg) << 10) & 0x000FFC00)
                                            | ((int) (b * fb * fb) & 0x000003FF);
                                }
                            }
                        }
                    }
                }
                txQ.add(frame);
                if (!(continueWithContent)) {
                    setSelectedContent(getSelectedContent() + 1);
                }
            } catch (Throwable t) {
                Log.severe(t);
            }
        }
    }

    private void reset() throws IOException, InterruptedException, PcapException {
        StringBuilder msg = new StringBuilder("Resetting Modules:");
        for (int m : config.getModules()) {
            msg.append(" ").append(m);
        }
        msg.append('\n');
        Log.info(msg.toString());
        for (int i = 0; i < 4; i++) {
            for (int m : config.getModules()) {
                byte[] packet = packet();
                PacketUtils.reset(packet, IConstants.UDP_PAYLOAD_OFF);
                send(packet, m);
            }
            Thread.sleep(200);
        }
        for (int m : config.getModules()) {
            byte[] packet = packet();
            PacketUtils.autoid(packet, IConstants.UDP_PAYLOAD_OFF);
            send(packet, m);
        }
        Thread.sleep(1000);
        Log.info("Reset done.");
    }

    private void getStatus() {
        byte[] packet = new byte[ADDR_LEN + PROT_LEN + DATA_LEN];
        AddressUtils.BROADCAST(packet, 0);
        AddressUtils.SELF(device, packet, 6);
        PacketUtils.status(packet, ADDR_LEN, 0);

        for (int i = 0; i < 5; i++) {
            try {
                device.send(packet);
                Thread.sleep(500);
                if (config.numOperational() > 0) {
                    return;
                }
                // Log.info("No modules found, retry " + (1 + i));
            } catch (Throwable t) {
                Log.severe(t);
            }
        }
    }

    private synchronized byte[] packet() {
        return packets[packetPtr++ % N_PACKET_BUFFERS];
    }

    private void send(byte[] packet, int module) throws IOException, PcapException {
        AddressUtils.MMUX(packet, 0, module);
        System.arraycopy(device.getAddr(), 0, packet, 6, 6);
        PacketUtils.UDP(packet, 12, selfIpAddr, selfIpPort, novaIpAddr[module], 3210, 1100);
        device.send(packet);
    }

    private void sync(int seqNum) {
        try {
            if (reset.getAndSet(false)) {
                novaOff();
                novaOn();
                return;
            }

            final int[][] frame = txQ.poll();
            final byte[] packet = packet();

            if (frame == null) {
                Log.info("Frame queue underrun");
                return;
            }

            for (int m : config.getModules()) {
                PacketUtils.rgb(packet, IConstants.UDP_PAYLOAD_OFF, seqNum + MODULE_QUEUE_SIZE, frame[m]);
                send(packet, m);
            }

            frameQ.add(frame);
        } catch (Throwable t) {
            Log.severe(t);
        }
    }

    private void readControlParams() {
        Properties props = new Properties();
        try (FileReader in = new FileReader(new File(properties.getProperty(PROPERTY_KEY_CONFIG_DIR, "."), CONTROL_PARAMS))) {
            props.load(in);
            setSelectedContent(Integer.parseInt(props.getProperty("selected-content", "" + selectedContent)));
            setHue(Float.parseFloat(props.getProperty("hue", "" + hue)));
            setSaturation(Float.parseFloat(props.getProperty("saturation", "" + saturation)));
            setBrightness(Float.parseFloat(props.getProperty("brightness", "" + brightness)));
            setSpeed(Float.parseFloat(props.getProperty("speed", "" + speed)));
        } catch (Throwable t) {
            //Log.severe(t);
        }
    }

    private void writeControlParams() {
        Properties props = new Properties();
        props.put("content", "" + selectedContent);
        props.put("hue", "" + hue);
        props.put("saturation", "" + saturation);
        props.put("brightness", "" + brightness);
        props.put("speed", "" + speed);
        try (FileWriter out = new FileWriter(new File(properties.getProperty(PROPERTY_KEY_CONFIG_DIR, "."), CONTROL_PARAMS))) {
            props.store(out, "NOVA control parameters");
        } catch (Throwable t) {
            Log.severe(t);
        }
    }
}
