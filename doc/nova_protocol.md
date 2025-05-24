## 1. Overview

Nova hardware drives modules at **50 Hz** (20 ms frame period) over raw Ethernet + UDP.
There are two packet types in use:

- **Sync packets** (`EtherType 0x0810`) for timing, status polls/replies, and shift commands
- **Data packets** (`EtherType 0x0800` + UDP port 3210) for RGB pixel chains, reset, and auto‑ID

---

## 2. Sync Packet (`EtherType 0x0810`)

```text
Offset  Len  Field           Value / Meaning
------- ---- --------------- -----------------------------------------------
0–5     6    DST MAC         Broadcast (FF:FF:FF:FF:FF:FF)
6–11    6    SRC MAC         Host interface MAC
12–13   2    EtherType       0x0810
14–15   2    Payload Length  Big‑endian = 46 (0x002E)
16      1    Command         0x00 = SYNC, 0x04 = STATUS
17      1    Status flag     Reserved (0)
18      1    Sequence number 8‑bit (wraps at 255)
19      1    Shift flag      1 = shift (no pixel UDP), 0 = send (pixel UDP follows)
20–45   26   Reserved        All zeros
```

- **Packet size**: 6 + 6 + 2 + 46 = 60 bytes
- **Usage**:
  - Every 20 ms: send SYNC with `Shift flag` toggling each frame
  - Every 5 s: send STATUS (`Command=0x04`) to poll modules
  - Modules reply with `Command=0x04`, DST MAC = host MAC

---

## 3. Module Reset & Auto‑ID

1. **Reset**: 4 rounds of UDP `UDP_CMD_RESET (0x00)`, 200 ms between rounds
2. **Auto‑ID**: 1 round of UDP `UDP_CMD_AUTOID (0x70)`, 200 ms after

_All use the same UDP payload format (§4) with `sequence=0`, `shift=false`._

---

## 4. Data Packet (`EtherType 0x0800` / UDP 3210)

### Ethernet + IP + UDP headers

```text
Offset  Len  Field           Value / Meaning
------- ---- --------------- -----------------------------
0–5     6    DST MAC         NOVA_MAC_PREFIX (00:20:E3:10:00) + module_address
6–11    6    SRC MAC         Host interface MAC
12–13   2    EtherType       0x0800 (IPv4)
14      1    IP ver + IHL    0x40|0x05 → v4, IHL=5
15      1    DSCP/ECN        0x00
16–17   2    Total Length    20+8+payload (≈1124)
18–19   2    ID              0x321c
20–21   2    Flags+Offset    0x4000 (DF)
22      1    TTL             128
23      1    Protocol        17 (UDP)
24–25   2    Checksum        header checksum
26–29   4    SRC IP          127.0.0.1
30–32   3    DST IP prefix   192.168.1
33      1    DST IP last     module_address
34–35   2    SRC UDP port    1234
36–37   2    DST UDP port    3210
38–39   2    UDP length      8 + payload (1100)
40–41   2    UDP checksum    0x0000
```

### Payload (25 chains × 44 B = 1100 B)

For each `chain` in 0…24:

```text
Byte  Field            Value / Meaning
----- ---------------- ---------------------------------
0     Marker           0xC0
1     Command          0x02=RGB, 0x00=RESET, 0x70=AUTOID
2     Sequence number  8-bit (wraps at 255)
3     Chain index      chain
4–43  Pixels           10×4 B packed RGB
```

- **Pixel packing**:
  ```text
  r10 = round(clamp(r,0,1)*1023)
  g10 = round(clamp(g,0,1)*1023)
  b10 = round(clamp(b,0,1)*1023)
  packed = (r10<<20)|(g10<<10)|b10
  ```
  — stored big‑endian in 4 B
- **Flip**: if `flip=true`, index i → 9 − i

---

## 5. Timing & Framing

```rust
const SYNC_PERIOD           = Duration::from_millis(20);
const SYNC_BUSY_WAIT_MARGIN = Duration::from_millis(5);
const STATUS_PERIOD         = Duration::from_millis(5000);
```

**Loop**:

1. Wait for next 20 ms tick (sleep until ~5 ms before, then busy‑wait)
2. Send sync packet with current `sequence_number` and `shift_pixels = (sync_mode == ShiftPixels)`
3. If `SendPixels`:
   - Render image
   - Send UDP RGB to each module (`UDP_CMD_RGB=0x02`) with `sequence+1`
   - `sync_mode = ShiftPixels`
4. Else (`ShiftPixels`):
   - `sequence_number = sequence_number.wrapping_add(1)`
   - Hardware shifts pixel buffer (no UDP)
   - `sync_mode = SendPixels`

Every 5 s: send STATUS to poll modules.

---

## 6. Module Status

On recv sync‑packet (`Command=0x04`):

- Verify length ≥ `NOVA_PACKET_LEN`
- Verify IP src prefix == `NOVA_IP_PREFIX`
- Record `Instant::now()`
- Module is “ready” if seen within last 5 s

---

## 7. Constants & Codes (including commented-out entries)

```rust
// Ethernet / IP / UDP
const ETHER_TYPE_IP:      u16   = 0x0800;
const ETHER_TYPE_NOVA:    u16   = 0x0810;

// Packet lengths
const NOVA_DATA_LEN:         usize = 46;
const NOVA_PACKET_LEN:       usize = 6 + 6 + 2 + NOVA_DATA_LEN; // 60
const IP_HEADER_LEN:         usize = 20;
const UDP_HEADER_LEN:        usize = 8;
const UDP_CHAINED_DATA_LEN:  usize = 25 * (4 + 10 * 4);         // 25×44 = 1100
const UDP_PACKET_LEN:        usize = 42 + UDP_CHAINED_DATA_LEN; // 1142

// Timeouts & periods
const SYNC_PERIOD:            Duration = Duration::from_millis(20);
const SYNC_BUSY_WAIT_MARGIN:  Duration = Duration::from_millis(5);
const STATUS_PERIOD:          Duration = Duration::from_millis(5_000);
const INTERFACE_RETRY_PERIOD: Duration = Duration::from_millis(100);

// Nova packet command values
const NOVA_CMD_SYNC:    u8 = 0x00;
// const NOVA_CMD_PLL:     u8 = 0x01;
// const NOVA_CMD_START:   u8 = 0x02;
// const NOVA_CMD_STOP:    u8 = 0x03;
const NOVA_CMD_STATUS:  u8 = 0x04;

// Status flags
// const NOVA_STATUS_STOPPED: u8 = 0x00;
// const NOVA_STATUS_RUNNING: u8 = 0x01;

// UDP packet command values
const UDP_CMD_RESET:    u8 = 0x00;
const UDP_CMD_RGB:      u8 = 0x02;
// const UDP_CMD_DOT_CORR:     u8 = 0x04;
// const UDP_CMD_COLOR_CORR:   u8 = 0x08;
// const UDP_CMD_BRIGHTNESS:   u8 = 0x10;
// const UDP_CMD_OPMODE:       u8 = 0x40;
const UDP_CMD_AUTOID:   u8 = 0x70;

// FSS Power flags (commented-out)
// const FSS_POWER_RESTART_PENDING:  u8 = 0x01;
// const FSS_POWER_PROGRAM_PENDING:  u8 = 0x02;
// const FSS_POWER_ERASE_PENDING:    u8 = 0x04;
// const FSS_POWER_PROGRAM_TIMEOUT:  u8 = 0x08;
// const FSS_POWER_ERASE_TIMEOUT:    u8 = 0x10;
// const FSS_POWER_OK1:              u8 = 0x20;
// const FSS_POWER_OK2:              u8 = 0x40;
// const FSS_POWER_OK3:              u8 = 0x80;

// Addresses & ports
const BROADCAST_MAC:   [u8; 6] = [0xFF; 6];
const NOVA_MAC_PREFIX: [u8; 5] = [0x00, 0x20, 0xE3, 0x10, 0x00];
const LOCAL_IP:        [u8; 4] = [127, 0, 0, 1];
const NOVA_IP_PREFIX:  [u8; 3] = [192, 168, 1];
const LOCAL_UDP_PORT:  u16     = 1234;
const NOVA_UDP_PORT:   u16     = 3210;

// IP header constants
const IP_VERSION:      u8   = 0x40; // + IHL=5 words (20 B)
```
