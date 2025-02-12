package org.corebounce.nova;

import java.io.IOException;
import java.util.concurrent.LinkedBlockingQueue;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;
import org.jnetpcap.BpFilter;
import org.jnetpcap.Pcap;
import org.jnetpcap.PcapException;
import org.jnetpcap.PcapHandler.OfArray;
import org.jnetpcap.PcapHeader;
import org.jnetpcap.PcapIf;

public final class EnetInterface implements IConstants {

  private static final int SEND_DELAY = 0;

  private final PcapIf device;
  private Pcap pcap;
  private final byte[] addr;
  private final LinkedBlockingQueue<byte[]> queue = new LinkedBlockingQueue<>();
  private final AtomicBoolean close = new AtomicBoolean();

  public EnetInterface(PcapIf device) throws IOException {
    this.device = device;
    this.addr = device.hardwareAddress().orElse(new byte[6]);
  }

  private EnetInterface() {
    this.device = null;
    this.addr = new byte[6];
  }

  public void open() throws IOException, PcapException {
    if (device != null) {
      int snaplen = 64 * 1024;
      int timeout = 1;
      pcap = Pcap.openLive(device, snaplen, true, timeout, TimeUnit.MILLISECONDS);
      if (pcap == null) {
        throw new IOException("Could not open device " + this);
      }

      String expression = "ether proto " + PROT_SYNC + " and ether dst " + toEnet(addr) + " or ether broadcast";
      BpFilter filter = pcap.compile(expression, false);
      pcap.setFilter(filter);
      new RxThread();
    }
  }

  public void close() throws InterruptedException {
    if (device != null) {
      close.set(true);
      while (!(close.get())) {
        Thread.sleep(5);
      }
    }
  }

  @SuppressWarnings("unused")
  public void send(byte[] packet) throws IOException, PcapException {
    if (pcap != null) {
      pcap.sendPacket(packet);
    }
    if (SEND_DELAY > 0) {
      try {
        Thread.sleep(SEND_DELAY);
      } catch (Throwable t) {
        throw new IOException(t);
      }
    }
  }

  public byte[] receive() throws InterruptedException {
    return queue.take();
  }

  public String getName() {
    return device != null ? device.name() : "dummy";
  }

  public byte[] getAddr() {
    return addr;
  }

  public boolean isDummy() {
    return device == null;
  }

  @Override
  public String toString() {
    return getName() + " (" + toEnet(addr) + ")";
  }

  static EnetInterface getInterface(String name) throws IOException, PcapException {
    for (var device : Pcap.findAllDevs()) {
      if (device.name().equals(name)) {
        return new EnetInterface(device);
      }
    }
    Log.warning("No such interface: " + name + " (using dummy)");
    return new EnetInterface();
  }

  private static String toEnet(byte[] addr) {
    if (addr == null || addr.length != 6) {
      throw new IllegalArgumentException("Invalid MAC address");
    }
    return String.format("%02x:%02x:%02x:%02x:%02x:%02x", addr[0], addr[1], addr[2], addr[3], addr[4], addr[5]);
  }

  private final class RxThread implements OfArray<LinkedBlockingQueue<byte[]>> {

    RxThread() {
      Thread t = new Thread(this::dispatch, "Nova Pcap Receive");
      t.setDaemon(true);
      t.start();
    }

    void dispatch() {
      if (pcap != null) {
        while (!(close.get())) {
          try {
            pcap.dispatch(1, this, queue);
          } catch (PcapException e) {
            Log.error(e);
          }
        }
        pcap.close();
      }
      pcap = null;
      close.set(false);
    }

    @Override
    public void handleArray(LinkedBlockingQueue<byte[]> user, PcapHeader header, byte[] packet) {
      queue.offer(packet.clone());
    }
  }
}
