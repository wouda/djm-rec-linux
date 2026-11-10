# Pro DJ Link Protocol Notes & Hardware Spike Findings

This document records protocol observations, packet captures, and hardware behavior for the Pioneer / AlphaTheta Pro DJ Link network, based on Deep Symmetry's *dysentery* research, packet analysis, and tests against physical Pioneer hardware (DJM-A9, DJM-V10, DJM-900NXS2, CDJ-3000, and CDJ-2000NXS2).

---

## 1. Physical & Network Layer Architecture

- **Topology**: Switched Ethernet (100Base-TX or 1000Base-T). Mixers (DJM-V10, DJM-A9) contain internal switches; external switches must not filter UDP broadcasts.
- **Addressing**: Link devices use static IPs in `192.168.85.0/24` or link-local auto-IP (`169.254.0.0/16`) if no DHCP server is present.
- **Standard Ports**:
  - `UDP 50000`: Device Announce / Keep-Alive discovery (broadcast)
  - `UDP 50001`: Player number claim & arbitration (broadcast)
  - `UDP 50002`: Real-time status stream (CDJ play state, pitch, On-Air channel mask from mixer)
  - `TCP 12523`: Database query server (Pioneer DB server for metadata queries)
  - `TCP/UDP 111` & `TCP/UDP 2049`: Sun RPC / NFS server exporting USB media on CDJs (`/media/...`)

---

## 2. Wire Packet Structure & Magic Header

All Pro DJ Link UDP packets share a 10-byte magic header:
```
Hex:    51 73 70 74 31 57 6d 6a 51 73
ASCII:  Q  s  p  t  1  W  m  j  Q  s
```
Following the magic header:
- `Offset 0x0A..0x1E` (20 bytes): Device name, ASCII, null-padded (e.g. `"DJM-A9\0\0..."`, `"CDJ-3000\0\0..."`)
- `Offset 0x1E` (1 byte): Packet category / sub-type
- `Offset 0x24` (1 byte): Source Player Number (`0x01`–`0x06`, `0x11`–`0x14` for mixers)

### Annotated Wire Captures

#### A. Keep-Alive / Announcement Packet (Port 50000, Broadcast)
Sent every 250–300 ms by all active link devices:
```hex
0000   51 73 70 74 31 57 6d 6a 51 73 44 4a 4d 2d 41 39   Qspt1WmjQsDJM-A9
0010   00 00 00 00 00 00 00 00 00 00 00 00 00 00 01 02   ..............0102
0020   00 00 00 01 11 00 00 00 c0 a8 55 11 00 00 00 00   ......c0a85511....
```
- `0x00..0x09`: Magic bytes `Qspt1WmjQs`
- `0x0A..0x1D`: Model string: `"DJM-A9"`
- `0x1E`: Packet type `0x06` (Announce) / `0x01` (Initial boot)
- `0x24`: Device index / player number (`0x11` = Mixer Ch 1 / main unit)
- `0x28..0x2B`: Device IP address `192.168.85.17` (`0xc0 0xa8 0x55 0x11`)

#### B. Mixer Status / On-Air Packet (Port 50002, Broadcast)
Broadcast by DJM mixers at ~20–30 Hz:
```hex
0000   51 73 70 74 31 57 6d 6a 51 73 44 4a 4d 2d 41 39   Qspt1WmjQsDJM-A9
0010   00 00 00 00 00 00 00 00 00 00 00 00 00 00 04 00   ..............0400
0020   00 00 00 01 11 00 00 00 00 00 00 00 0f 00 00 00   ................
```
- `0x1E`: Subtype `0x04` (Mixer Status)
- `0x2C`: **On-Air Channel Bitmask**:
  - Bit 0 (`0x01`): Channel 1 On-Air (Fader up, Master unmuted)
  - Bit 1 (`0x02`): Channel 2 On-Air
  - Bit 2 (`0x04`): Channel 3 On-Air
  - Bit 3 (`0x08`): Channel 4 On-Air
  - Bit 4 (`0x10`): Channel 5 On-Air (V10)
  - Bit 5 (`0x20`): Channel 6 On-Air (V10)
  - *Example*: Value `0x06` (`0000 0110`b) indicates Channels 2 and 3 are currently On-Air.

#### C. CDJ Deck Status Packet (Port 50002, Port 50002 Unicast/Broadcast)
Broadcast by CDJs at ~30 Hz:
```hex
0000   51 73 70 74 31 57 6d 6a 51 73 43 44 4a 2d 33 30   Qspt1WmjQsCDJ-30
0010   30 30 00 00 00 00 00 00 00 00 00 00 0a 00 00 00   00............0a00
0020   00 00 00 01 02 00 00 00 00 00 03 e9 00 00 00 00   ....02......03e9....
...
0080   00 00 00 00 00 00 00 00 00 02 00 00 00 00 00 00   ................
```
- `0x1E`: Subtype `0x0a` (Player Status)
- `0x24`: Player number (`0x02` = Player 2)
- `0x28..0x2B`: Rekordbox Track ID (32-bit big endian: `0x000003e9` = Track 1001)
- `0x89`: Play status (`0x02` = Playing, `0x00` = Paused/Stopped, `0x03` = Cued)
- `0x92`: Master deck indicator (`0x01` = Master)

---

## 3. Hardware Spike Findings: Player Number 5 & 6 Compatibility (Task 4.2)

### 6-Player Capable Networks (DJM-V10, DJM-A9 with CDJ-3000)
- **Status Packet Routing**: Physical CDJ-3000s and DJM-V10 support 6 players natively. Status packets and On-Air tallies on channels 5 and 6 work identically to channels 1–4.
- **Virtual Participation**: Claiming virtual player number 6 or 5 on a 6-player network causes CDJ-3000s to recognize the recording box as a link participant and supply loaded track IDs over port 50002.
- **Arbitration Rule**: `djm-prodj` chooses the highest available number (slot 6, fallback to slot 5).

### 4-Player Networks (DJM-900NXS2, CDJ-2000NXS2)
- **Status Packet Routing**: NXS2 players only route database queries and metadata to player numbers 1, 2, 3, and 4. Packets claiming numbers 5 or 6 are ignored by NXS2 player database query servers.
- **Participation Rule for 4-Player Networks**:
  - `djm-prodj` must NOT claim numbers 5 or 6 on 4-player networks.
  - `djm-prodj` must start **completely passive**.
  - Only if the network has ≤3 physical players AND live audio has been playing for at least 3 minutes with zero new device announcements, `djm-prodj` may claim player number 4.
  - If 4 physical players are connected, `djm-prodj` remains permanently passive (degraded mode).

---

## 4. Hardware Spike Findings: USB Metadata & Media Resolution (Task 4.3)

### Reading `export.pdb` & `DEVSETTING.DAT` Without a Player Number
- **NFS Protocol Analysis**:
  - CDJs run an unauthenticated Sun RPC / NFS server on ports 111 / 2049.
  - When a USB thumb drive is inserted, the CDJ exports the volume at `/media/USB_PORT/` or `/media/sdb1/`.
  - **Result**: Any host on the Ethernet subnet can query NFS `MOUNT` and read files from the player *without* registering or holding a player number on the Pro DJ Link network!
  - `export.pdb` (DeviceSQL / rekordbox export database) can be copied over NFS or read directly.
  - `DEVSETTING.DAT` contains the USB drive's unique volume label and serial/UUID.
- **Parsing with `rekordcrate`**:
  - The `rekordcrate` library successfully parses `export.pdb` binary files, mapping track IDs to:
    - Title, Artist, Album, Genre, BPM, Initial Key, Duration, Date Added.
- **Degraded Mode Decision**:
  - If NFS access fails or times out, or when running passively, `djm-prodj` logs On-Air durations, deck numbers, and track ID hashes, marking artist/title as `"Unavailable (Degraded Mode)"`.
  - The recording and split engine remains 100% operational in degraded mode.

---

## 5. Participation State Machine & Release Latency Rules

```
                      +-------------------+
                      |      Passive      |
                      | (Listen Only)     |
                      +---------+---------+
                                |
             Ethernet settled   | >30s no new devices
             Live audio active  | >3m continuous music
             Free slot exists   | (Slot 6/5 on 6p, Slot 4 on 4p)
                                v
                      +-------------------+
                      |  Virtual Player   |
                      |  (Slot Claimed)   |
                      +---------+---------+
                                |
             ANY new device packet seen OR
             Another device claims our slot
                                |
                                v
                      +-------------------+
                      | Immediate Silent  |
                      |     Release       |
                      +-------------------+
```

### Zero-Contest Guarantee:
1. `djm-prodj` **never** transmits an announcement defending its number if an incoming packet claims that number.
2. The instant any packet is received with an unknown MAC or claiming our number, `djm-prodj` immediately stops all transmissions, cancels recurring UDP timers, and transitions back to `Passive`.
3. Release latency: **0 additional packets**.
