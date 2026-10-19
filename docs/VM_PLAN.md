# Reference environment: Windows VM for PE-DESIGN 11

Goal: run a licensed PE-DESIGN 11 in an isolated Windows VM and capture reproducible reference
outputs (files, screenshots, parameter records) for COMPATIBILITY_TESTS.md.

## 1. Virtualisation choice (Apple Silicon)

Only ARM Windows runs on Apple Silicon. Windows 11 on ARM runs x86/x64 apps such as PE-DESIGN
through Microsoft's built-in emulation (Prism). Brother does not list Windows on ARM as a supported
platform, so whether PE-DESIGN works there is **unverified until the first install**.

| Option | Cost | Pros | Cons |
|---|---|---|---|
| **Parallels Desktop** (recommended) | Paid subscription | Microsoft-authorised for Windows 11 ARM; best USB passthrough (Brother machine / USB stick); snapshots; shared folders; `prlctl` CLI for scripting | Cost |
| UTM (QEMU) | Free | Open source; snapshots | Weaker USB passthrough and graphics; more setup |
| VMware Fusion | Free for personal use | Snapshots; USB | Windows ARM support less polished |

Windows 11 ARM needs its own licence for long-term use (Parallels can download it).

## 2. Build steps

1. Create a VM: Windows 11 ARM, 4 vCPU, 8 GB RAM, 80 GB disk. Isolation settings: **no**
   shared clipboard, **no** mapping of the Mac home folder, **one** shared folder only:
   `ope-design11/reference/` (git-ignored), mounted read-write.
2. Windows Update; leave Defender on; take snapshot **`00-clean-os`**.
3. Install a licensed PE-DESIGN 11 from Brother's official download, apply Brother's latest
   update, and register it with your own licence through Brother's normal process.
4. Record the installed version and the file associations it registers
   (`HKCR\.*` → `reference/inventory/associations.txt`) as background for the format list.
5. Snapshot **`10-pedesign-ready`**. Every experiment starts by reverting to this snapshot,
   so default settings are identical between runs.

## 3. Reference capture protocol

For each experiment `EXP-NNN` (defined in COMPATIBILITY_TESTS.md):

```
reference/EXP-NNN/
  input/            files fed to PE-DESIGN (our own designs, images, fonts we hold rights to)
  steps.md          exact UI steps, menu paths, every parameter value changed from default
  output/           files saved/exported by PE-DESIGN (PES each version, DST, EXP, JEF, VP3, …)
  screenshots/      key dialogs and the result (for UI/workflow documentation only)
  meta.json         PE-DESIGN version, Windows build, date, hoop, units
```

Rules: change one parameter per experiment where possible; keep the UI language English;
units mm; never feed purchased/third-party designs into fixtures that leave your machine.

## 4. Initial experiment batch (formats first)

| ID | Experiment | Unlocks |
|---|---|---|
| EXP-001 | Import `oracle.dst` (our file) → save as PES in every offered version | PES version differences; how PE-DESIGN stores imported stitches |
| EXP-002 | Import `oracle.pes` (OpenStitch-written) → confirm it opens; re-export as PES/DST/EXP/JEF/VP3/PHC | Confirms our writer; gives cross-format fixtures |
| EXP-003 | Draw one rectangle, running stitch only, default parameters → save PES; vary stitch length 2.0/2.5/3.0 mm | First stitch-generation spec |
| EXP-004 | Same rectangle as satin column, default → vary density and underlay | Satin spec |
| EXP-005 | Same rectangle as fill, default → vary direction 0/45/90° and density | Fill spec |
| EXP-006 | Single letter "A", default font, 10/20/30 mm | Lettering behaviour (font outlines not reused) |
| EXP-007 | List File → Export / Save As format menus and Import filters (screenshots) | Authoritative supported-format list |
| EXP-008 | Sew-out: OpenStitch-written PES on the owner's Brother machine (USB stick) | Machine compatibility |

## 5. Automation (later)

Once the manual protocol works, script repeatable experiments inside the VM with
pywinauto or AutoHotkey driving the UI (no code inspection or patching of PE-DESIGN), and run
them with `prlctl exec`. Host-side `stitchtool` then diffs `output/` against OpenStitch's
results.

## 6. Out of bounds

- No debuggers, API hooking or memory inspection of PE-DESIGN.
- No modification of PE-DESIGN files, licence data or network traffic.
