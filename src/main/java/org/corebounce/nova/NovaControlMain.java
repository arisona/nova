package org.corebounce.nova;

import java.io.IOException;
import java.net.DatagramSocket;
import java.net.Inet4Address;
import java.net.SocketException;
import java.util.concurrent.LinkedBlockingQueue;
import java.util.concurrent.atomic.AtomicBoolean;
import org.jnetpcap.PcapException;

public final class NovaControlMain implements IConstants {

  private static final int N_PACKET_BUFFERS = 1024;
  private static final int MODULE_QUEUE_SIZE = 4;
  private static final int FRAME_QUEUE_SIZE = MODULE_QUEUE_SIZE + 4;

  private static NovaControlMain theControl;

  private final State state;
  private final EnetInterface device;

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

  public NovaControlMain() throws SocketException, IOException, PcapException {
    if (theControl != null) {
      throw new RuntimeException("Cannot instantiate multiple NOVAControl instances.");
    }
    theControl = this;

    state = new State();

    device = EnetInterface.getInterface(state.getEthernetInterface());
    Log.info("Using interface: " + device.getName());
    device.open();

    try (DatagramSocket socket = new DatagramSocket()) {
      selfIpAddr = Inet4Address.getLocalHost().getAddress();
      selfIpPort = socket.getLocalPort();
    }

    int maxModule = 0;
    for (int m : state.getModules()) {
      novaIpAddr[m] = new byte[] { (byte) NOVA_IP_0, (byte) NOVA_IP_1, (byte) NOVA_IP_2, (byte) m };
      maxModule = Math.max(maxModule, m);
    }

    for (int i = 0; i < FRAME_QUEUE_SIZE; i++) {
      frameQ.add(new int[maxModule + 1][state.getDimI() * state.getDimJ() * state.getDimK() * 3]);
    }

    dispatcher = new Dispatcher(device, state);

    new UIServer(state);

    Thread thread = new Thread(this::streamTask, "Voxel Streamer");
    thread.setPriority(Thread.MIN_PRIORITY);
    thread.start();

    for (;;) {
      try {
        getStatus();
        Log.info("NOVA status: " + state.getNumOperational() + " of " + state.getNumModules() + " operational");
        if (state.isOperational()) {
          if (!(isOn())) {
            novaOn();
          }
        } else {
          // note: not sure if we want this (if nova behaves unstable, just comment out)
          if (isOn()) {
            novaOff();
            Log.info("NOVA was switched off: exiting");
            System.exit(0);
          }
        }
        Thread.sleep(isOn() ? 10000 : 1000);
      } catch (Throwable t) {
        Log.error(t);
      }
    }
  }

  static NovaControlMain get() {
    return theControl;
  }

  public static void main(String[] args) throws IOException, InterruptedException, PcapException {
    new NovaControlMain();
  }

  boolean isOn() {
    return syncGen != null;
  }

  State getState() {
    return state;
  }

  EnetInterface getDevice() {
    return device;
  }

  void novaOn() throws IOException, InterruptedException, PcapException {
    if (!(isOn()) && device != null) {
      Log.info("Nova on");
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
      Log.info("Nova off");
      syncGen.setListener(null);
      syncGen.dispose();
      syncGen = null;
    }
  }

  void novaReset() {
    reset.set(true);
  }

  private void streamTask() {
    final float[] fframe = new float[state.getDimI() * state.getDimJ() * state.getDimK() * 3];
    int selectedContentIndex = -1;
    int frameCount = 0;

    // note: the speed calculation is a bit weird, but made to match the original
    // implementation where speed was in the range [-3, 5] and now is [0, 1]
    for (double time = 0;; time += 0.04 * Math.pow(2, state.getSpeed() * 8 - 3)) {
      try {
        int newContentIndex = state.getSelectedContentIndex();
        if (state.getCycleDuration() > 0 && frameCount++ >= state.getCycleDuration() * 25) {
          newContentIndex = getNextEnabledContentIndex(selectedContentIndex);
          frameCount = 0;
        }
        if (newContentIndex != selectedContentIndex) {
          if (selectedContentIndex >= 0) {
            try {
              state.getContent(selectedContentIndex).stop();
            } catch (Throwable t) {}
          }
          selectedContentIndex = newContentIndex;
          if (selectedContentIndex >= 0) {
            Log.info("Set content " + selectedContentIndex + ": " + state.getContent(selectedContentIndex));
            try {
              state.getContent(selectedContentIndex).start();
            } catch (Throwable t) {}
          } else {
            Log.info("Set content -1: blank");
          }
        }
        int[][] frame = frameQ.take();
        if (selectedContentIndex < 0) {
          for (int i = 0; i < frame.length; i++) {
            for (int j = 0; j < frame[i].length; j++) {
              frame[i][j] = 0;
            }
          }
          txQ.add(frame);
          continue;
        }

        state.getContent(selectedContentIndex).fillFrame(fframe, time);

        float[] rgb = ColorUtils.hsvToRgb(
          state.getHue(),
          state.getSaturation(),
          state.getBrightness() * state.getBrightness()
        );
        float r = rgb[0] * 1023;
        float g = rgb[1] * 1023;
        float b = rgb[2] * 1023;

        for (int m : state.getModules()) {
          int off = state.getFrameOffset(m);
          int idx = 0;

          int[] pixels = frame[m];
          if (state.isFlipVertical()) {
            for (int i = state.getModuleDimI(); i-- > 0;) {
              for (int j = state.getModuleDimJ(); j-- > 0;) {
                for (int k = 0; k < state.getModuleDimK(); k++) {
                  int x =
                    off +
                    3 * (j * state.getDimI() * state.getDimK() + i * state.getDimK() + ((state.getDimK() - 1) - k));
                  float fr = fframe[x];
                  float fg = fframe[x + 1];
                  float fb = fframe[x + 2];
                  pixels[idx++] = (((int) (r * fr * fr) << 20) & 0x3FF00000) |
                  (((int) (g * fg * fg) << 10) & 0x000FFC00) |
                  ((int) (b * fb * fb) & 0x000003FF);
                }
              }
            }
          } else {
            for (int i = state.getModuleDimI(); i-- > 0;) {
              for (int j = state.getModuleDimJ(); j-- > 0;) {
                for (int k = 0; k < state.getModuleDimK(); k++) {
                  int x = off + 3 * (j * state.getDimI() * state.getDimK() + i * state.getDimK() + k);
                  float fr = fframe[x];
                  float fg = fframe[x + 1];
                  float fb = fframe[x + 2];
                  pixels[idx++] = (((int) (r * fr * fr) << 20) & 0x3FF00000) |
                  (((int) (g * fg * fg) << 10) & 0x000FFC00) |
                  ((int) (b * fb * fb) & 0x000003FF);
                }
              }
            }
          }
        }
        txQ.add(frame);
      } catch (Throwable t) {
        Log.error(t);
      }
    }
  }

  private int getNextEnabledContentIndex(int selectedContentIndex) {
    try {
      int index = state.getEnabledContentIndices().nextSetBit(selectedContentIndex + 1);
      if (index == -1) index = state.getEnabledContentIndices().nextSetBit(0);
      return index;
    } catch (IndexOutOfBoundsException e) {
      return -1;
    }
  }

  private void reset() throws IOException, InterruptedException, PcapException {
    StringBuilder msg = new StringBuilder("Resetting modules:");
    for (int m : state.getModules()) {
      msg.append(" ").append(m);
    }
    msg.append('\n');
    Log.info(msg.toString());
    for (int i = 0; i < 4; i++) {
      for (int m : state.getModules()) {
        byte[] packet = packet();
        PacketUtils.reset(packet, IConstants.UDP_PAYLOAD_OFF);
        send(packet, m);
      }
      Thread.sleep(200);
    }
    for (int m : state.getModules()) {
      byte[] packet = packet();
      PacketUtils.autoid(packet, IConstants.UDP_PAYLOAD_OFF);
      send(packet, m);
    }
    Thread.sleep(1000);
    Log.info("Reset complete");
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
        if (state.getNumOperational() > 0) {
          return;
        }
        // Log.info("No modules found, retry " + (1 + i));
      } catch (Throwable t) {
        Log.error(t);
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
        Log.error("Frame queue underrun");
        return;
      }

      for (int m : state.getModules()) {
        PacketUtils.rgb(packet, IConstants.UDP_PAYLOAD_OFF, seqNum + MODULE_QUEUE_SIZE, frame[m]);
        send(packet, m);
      }

      frameQ.add(frame);
    } catch (Throwable t) {
      Log.error(t);
    }
  }
}
