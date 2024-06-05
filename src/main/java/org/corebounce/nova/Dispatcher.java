package org.corebounce.nova;

import org.corebounce.util.Log;

public final class Dispatcher implements IConstants {

  final EnetInterface device;
  final State state;
  SyncGenerator sync;

  public Dispatcher(EnetInterface device, State state) {
    this.device = device;
    this.state = state;
    if (!device.isDummy()) {
      Thread thread = new Thread(this::dispatchTask);
      thread.setPriority(Thread.MIN_PRIORITY);
      thread.start();
    }
  }

  public void setSyncGen(SyncGenerator sync) {
    this.sync = sync;
  }

  private void dispatchTask() {
    byte[] status = new byte[ADDR_LEN + PROT_LEN + DATA_LEN];
    AddressUtils.SYNC(status, 6);
    for (;;) {
      try {
        byte[] packet = device.receive();
        if (!(PacketUtils.isNOVAEnet(packet))) {
          continue;
        }
        if (AddressUtils.isDstBroadcast(packet)) {
          continue;
        }
        synchronized (this) {
          System.arraycopy(packet, 6, status, 0, 6);
          switch (packet[ADDR_LEN + PROT_LEN + 2]) {
            case CMD_START -> {
              if (sync != null) {
                sync.handle(CMD_START, status);
              }
            }
            case CMD_STOP -> {
              if (sync != null) {
                sync.handle(CMD_STOP, status);
              }
            }
            case CMD_STATUS -> {
              if (packet[20] == (byte) NOVA_IP_0) {
                if (state != null) {
                  state.setStatus(new DMUXStatus(packet, ADDR_LEN + PROT_LEN));
                }
              } else if (sync != null) {
                sync.handle(CMD_STATUS, status);
              }
            }
          }
        }
      } catch (Throwable t) {
        Log.severe(t);
      }
    }
  }
}
