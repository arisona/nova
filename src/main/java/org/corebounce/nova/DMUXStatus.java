package org.corebounce.nova;

public final class DMUXStatus {

    public final long timestamp;
    public final int ipAddr;
    public final short temp0;
    public final short temp1;
    public final short temp2;
    public final short temp3;
    public final short tempLED;
    public final byte statErr0;
    public final byte statErr1;
    public final byte statErr2;
    public final byte statErr3;
    public final byte statErr4;
    public final byte statErr5;
    public final byte freeScaleStatus;

    private short readShort(byte[] packet, int off) {
        return (short) ((packet[off] & 0xFF) | ((packet[off] & 0xFF) << 8));
    }

    public DMUXStatus(byte[] packet, int off) {
        this.timestamp = System.currentTimeMillis();
        off++;   // skip length
        off++;   // skip command
        off += 4; // skip padding
        int addr = packet[off++] & 0xFF;
        addr <<= 8;
        addr |= packet[off++] & 0xFF;
        addr <<= 8;
        addr |= packet[off++] & 0xFF;
        addr <<= 8;
        addr |= packet[off++] & 0xFF;
        this.ipAddr = addr;
        this.temp0 = readShort(packet, off);
        off += 2;
        this.temp1 = readShort(packet, off);
        off += 2;
        this.temp2 = readShort(packet, off);
        off += 2;
        this.temp3 = readShort(packet, off);
        off += 2;
        this.tempLED = readShort(packet, off);
        off += 2;
        this.statErr0 = packet[off++];
        this.statErr1 = packet[off++];
        this.statErr2 = packet[off++];
        this.statErr3 = packet[off++];
        this.statErr4 = packet[off++];
        this.statErr5 = packet[off++];
        this.freeScaleStatus = packet[off++];
    }

    public boolean isOperational() {
        return System.currentTimeMillis() - timestamp < 30L * 1000L;
    }
}
