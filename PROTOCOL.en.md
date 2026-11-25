# Mountain Everest Max protocol

**English** · [Français](PROTOCOL.md)

How Base Camp talks to the Everest Max keyboard (USB ID `3282:0001`), packet by packet. I got this by disassembling `SDKDLL.dll` and reading the .NET metadata and IL of `BaseCamp.Service.exe` and `BaseCamp.UI.dll`.

## Notation

- Bytes in code font are hexadecimal (`14 2C`). Other numbers are decimal.
- Every packet is 64 bytes long. Bytes that aren't listed are `00`.
- `val_lo val_hi` is a 16-bit value with the low byte first.
- Command names such as ChangeEffect or SaveFlash are the SDK's own function names.

## Base Camp on Windows

`Base Camp.exe` is an Electron shell around `BaseCamp.UI.exe`, an ASP.NET app shipped as a single file with a SQLite database, `BaseCamp.db`. That UI sends JSON requests like `{'Class':'Everest','MethodName':…}` to `BaseCamp.Service.exe`, which in turn calls `SDKDLL.dll` to reach the keyboard.

"Computer" actions (launching a program, playing a sound, OBS, Spotify…) are run by the service when the key is pressed.

## USB transport

Use HID interface 3, the one with vendor usage page `0xFF00`. Reports are 64 bytes and have no report ID, so on Linux you write 65 bytes: a leading `00`, then the packet.

### Commands

You send an OUTPUT report and the keyboard answers with an INPUT report that starts with the same header (`11 14 …`). `FF AA` means the keyboard refused. The app waits up to 600 ms for a reply. Three commands get a longer wait: 950 ms for SwitchProfile, 1.3 s for SaveFlash and 2.8 s for writing the dock configuration.

### Events

The keyboard sends these INPUT reports on its own. Report `01` is a bitfield of every key, where D1 to D4 are bits 1 to 4 of byte 42. Reports `02` to `0A` are other events, so don't mistake them for a command reply.

### Image transfers

These use 64-byte feature reports (`HIDIOCSFEATURE` and `HIDIOCGFEATURE`). In the answer, byte 2 echoes the command and byte 3 is a status:

| Status | Meaning |
|---|---|
| `FA` | OK |
| `FB` | busy, try again |
| `FC` | OK |
| `FE` | error |

## Lighting

The app sets an effect in up to three steps:

1. SwitchProfile selects the profile and the effect's slot.
2. ChangeEffect sends the settings. The Custom effect uses ChangeCustomizeStatic here.
3. SaveFlash is sent only when the keyboard should remember the effect.

| Command | Packet |
|---|---|
| SwitchProfile(profile 1 to 5, slot 0 to 8) | `14 00 00 00 profile slot` |
| ChangeEffect(EffData) | `14 2C` followed by the 62 bytes of EffData |
| ChangeCustomizeStatic | 7 packets of `14 2C 00 01 n 4B 00` followed by 57 bytes |
| SaveFlash(slot) | `13 55 00 00 slot` |

An effect is known by two numbers. Its ID goes into EffData, and its slot goes to SwitchProfile and SaveFlash.

| Effect | ID | Slot |
|---|---|---|
| Static | 0 | 0 |
| Breathing | 1 | 3 |
| Reactive | 3 | 4 |
| Wave | 4 | 1 |
| Yeti | 6 | 7 |
| Tornado | 7 | 2 |
| Matrix | 9 | 5 |
| Custom | 10 | 6 |
| Off | 12 | 8 |

### EffData for most effects

Static, Breathing, Reactive, Yeti, Matrix and Off share this layout:

```
byte 0       effect ID
byte 1       byAll
byte 2       speed
byte 3       brightness
byte 4       randColor
byte 5       direction
byte 6       width
bytes 7-15   three RGB colors
bytes 16-18  background RGB color
bytes 19-61  00
```

- Speed is a firmware code, listed under "Speed" below. Static uses `FF`.
- randColor is 0 for one color, 16 for two colors and 2 for rainbow. Static takes 0 or 2, and Off uses `FF`.
- Direction and width are always `FF` for these six effects.

### EffData for Wave and Tornado

The SDK calls this variant ChangeBlockEffect, because the colors are stored as blocks:

```
bytes 0-4    effect ID, byAll, speed, brightness, randColor (same as above)
byte 5       direction
byte 6       width
byte 7       number of blocks
byte 8...    one 4-byte block per color: level R G B
```

Wave takes direction 6, 2, 4 or 0. One color is sent as a single block at level 100. Two colors become 4 blocks at levels 25, 50, 75 and 100. Rainbow sets the width to 2 and the level to `FF`.

Tornado takes direction 10 for clockwise, or 9.

### Speed

The UI offers a speed from 0 to 100. Before sending, the SDK turns it into a firmware code that depends on the effect:

| UI speed | 0 to 12 | 13 to 37 | 38 to 62 | 63 to 87 | 88 to 100 |
|---|---|---|---|---|---|
| Wave, Tornado | 10 | 9 | 8 | 7 | 6 |
| Breathing, Reactive | 5 | 4 | 3 | 1 | 0 |
| Matrix | 20 | 15 | 10 | 5 | 0 |
| Yeti | 10 | 7 | 5 | 3 | 0 |

### Per-key colors

The Custom effect (ID 10, byAll = 1) gets no EffData from the SDK. It only gets ChangeCustomizeStatic, which carries one RGB color for each of 126 keys, ordered by matrixId (the `matrix` field in `rust/data/keys.json`). That makes 378 bytes, split over 7 packets of 57 bytes, with `n` counting the packets from 0 to 6. The keyboard keeps up with about 47 frames per second.

## Keys

Everything in this section is saved inside the keyboard. `src` is the DLLKeyId of the key you are changing, which is the `dll` field in `rust/data/keys.json`.

| Command | Packet |
|---|---|
| ChangeKey(src, value) | `14 20 src 00 val_lo val_hi` |
| ChangeShortcutKey(src, key, mods) | `14 21 src 00 key mods` |
| Macro header | `14 18 mode 00 repeats` |
| Macro data | `15 src count more` followed by 4-byte entries |
| SetLaunchContent | `17 src len step` followed by 60 bytes |

### ChangeKey values

- 0 turns the key off.
- 255 restores its default behavior.
- Another key's DLLKeyId makes it act as that key.
- 180 to 188 are media functions.
- 257 to 261, 304 and 305 are mouse functions.
- 160 to 164 and 210 to 212 switch profiles.
- 128 is power off, 129 is sleep and 195 opens the calculator.

`rust/data/function_values.json` has the full list.

### Shortcuts

`mods` is the sum of the modifiers you want: Ctrl is 1, Shift 2, Alt 4 and Win 8.

### Macros

A macro starts with the header packet. Its mode is 1 for play once, 4 for play while held and 3 for toggle. I haven't confirmed those three labels.

Data packets come next. `count` says how many entries the packet holds, 15 at most. `more` is 0 in the first packet and 1 in every later one. Each entry is 4 bytes:

```
key   (page << 4) | action   delay_lo   delay_hi
```

| Field | Values |
|---|---|
| action | 2 press, 1 release, 3 wheel |
| page | 0 keyboard, 1 mouse |
| key, on the mouse page | 1 left, 2 right, 3 middle, 49 and 48 wheel |

The last entry is always `FF FF 00 00`, and it is included in `count`.

### Launch content

SetLaunchContent takes a `step` of 0, 1, 2 or 3. Its 60 bytes hold the "launch" content that the service reads.

## Images

Pictures for the display keys (D1 to D4) and for the dock are RGB565, little-endian, with rows from top to bottom.

| Target | Size |
|---|---|
| Display key | 72×72 |
| Dock screen | 240×204 |
| Dock icon | 128×32 |

Steps 1 to 3 are feature reports. A reply written as `21 FA` is read from bytes 2 and 3.

1. Send `AA 55 21 type`. The type is 3 for the dock and 4 for the numpad. Wait for `21 FA`.
2. Send `AA 55 10 size[3] sum[2] 00 00 mode bank sub`. Wait for `10 FA`. The size takes 3 bytes. The sum is every byte of the picture added together, kept to 16 bits.
3. Send the picture in 64-byte blocks. Each reply gives the status and, in bytes 4 to 6, how many bytes the keyboard has received. After the last block, wait about 0.6 s. The reply then has the sum in bytes 7 and 8.
4. If anything fails, send the command `1D 00` to leave transfer mode and start over.

On the numpad, mode is 2, bank is `FW_EXTEND_INFO.byMMDockShowProfile[profile-1]` (1 for profile 1) and `sub` is the key, 0 to 3. On the dock, mode is 2, bank 1 is the screen and banks 2 to 6 are the icons.

## Media dock

| Command | Packet |
|---|---|
| SetClockInfo | `11 84 00 01 00 00 month day hour min sec f1 f2` (the app sends `00 00` for f1 and f2) |
| SetVolumeInfo | `11 83 00 00 volume`, with volume from 0 to 100 |
| SetPCInfo(type, value) | `11 81 type 00 val_lo val_hi` |
| GetExtendInfo | `11 14`. The reply holds FW_EXTEND_INFO starting at byte 4. |
| Writing FW_EXTEND_INFO | `11 14 00 01` followed by 29 bytes |
| EraseSectorMem | `1B …` **Do not use.** |

### PC info

The dock shows 0 unless the computer sends SetPCInfo every second, once for each type:

| Type | Value |
|---|---|
| 0 | CPU, in % |
| 1 | GPU, in % |
| 2 | disk, in % |
| 3 | internet |
| 4 | RAM, in % |
| 5 | APM, up to 999 |

### Extended configuration (FW_EXTEND_INFO)

These 29 bytes hold the dock's menus, its screensaver and the list of profiles.

| Byte | Field |
|---|---|
| 1 | wheel menus, one bit each |
| 3 to 5 | menu color |
| 6 | screensaver: type in the high 4 bits, bit 1 for turn-off, bit 0 for screensaver |
| 7 and 8 | screensaver delay in seconds, 1800 at most |
| 9 and 10 | turn-off delay in seconds, 1800 at most |
| 11 to 15 | profiles: enabled profile numbers first, then disabled ones written as `0x10 + n` |

Menu bits, from 7 down to 0: custom, APM, PC info, brightness, volume, lighting, profile, clock.

Screensaver types: 0 image, 1 and 2 clock, 3 timer, 4 stopwatch, 7 volume, 8 brightness, 9 CPU, 10 GPU, 11 disk, 12 RAM, 13 internet, 14 APM.
