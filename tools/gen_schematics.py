#!/usr/bin/env python3
"""PetFilter KiCad Schematic Generator - generates 6 hierarchical .kicad_sch files."""
import os

outdir = "hardware/pcb"
os.makedirs(outdir, exist_ok=True)

R = "e63e39d7-6ac0-4ffd-8aa3-1841a4541b55"
SU = {
    "power": "a0000001-0000-4000-8000-000000000001",
    "mcu": "a0000002-0000-4000-8000-000000000002",
    "motor": "a0000003-0000-4000-8000-000000000003",
    "uvc": "a0000004-0000-4000-8000-000000000004",
    "sensors": "a0000005-0000-4000-8000-000000000005"
}

_c = [0]
def U():
    _c[0] += 1
    return "b%07d-0000-4000-8000-%012d" % (_c[0], _c[0])

_p = [0]
def PR():
    _p[0] += 1
    return "#PWR%03d" % _p[0]

def prop(n, v, x, y, r=0, h=False):
    hd = " hide" if h else ""
    return '    (property "%s" "%s" (at %.2f %.2f %d)\n      (effects (font (size 1.27 1.27))%s)\n    )' % (n, v, x, y, r, hd)

def sym(lid, ref, val, fp, x, y, rot=0, np=2, sp="/"):
    u = U()
    ps = [prop("Reference", ref, x, y-2.54, rot),
          prop("Value", val, x, y+2.54, rot),
          prop("Footprint", fp, x, y+5.08, rot, True),
          prop("Datasheet", "", x, y+7.62, rot, True)]
    pns = "\n".join('    (pin "%d" (uuid "%s"))' % (i+1, U()) for i in range(np))
    return ('  (symbol (lib_id "%s") (at %.2f %.2f %d) (unit 1)\n'
            '    (in_bom yes) (on_board yes) (dnp no)\n    (uuid "%s")\n' % (lid, x, y, rot, u)
            + "\n".join(ps) + "\n" + pns + "\n"
            + '    (instances (project "petfilter" (path "%s" (reference "%s") (unit 1))))\n  )' % (sp, ref))

def pwr(n, ref, x, y, rot=0, sp="/"):
    u = U()
    vy = y + (2.54 if rot == 180 else -2.54)
    return ('  (symbol (lib_id "power:%s") (at %.2f %.2f %d) (unit 1)\n'
            '    (in_bom no) (on_board no) (dnp no)\n    (uuid "%s")\n' % (n, x, y, rot, u)
            + prop("Reference", ref, x+2, y, 0, True) + "\n"
            + prop("Value", n, x, vy, 0) + "\n"
            + prop("Footprint", "", x, y, 0, True) + "\n"
            + prop("Datasheet", "", x, y, 0, True) + "\n"
            + '    (pin "1" (uuid "%s"))\n' % U()
            + '    (instances (project "petfilter" (path "%s" (reference "%s") (unit 1))))\n  )' % (sp, ref))

def W(x1, y1, x2, y2):
    return '  (wire (pts (xy %.2f %.2f) (xy %.2f %.2f))\n    (stroke (width 0) (type default))\n    (uuid "%s")\n  )' % (x1, y1, x2, y2, U())

def lbl(n, x, y, rot=0):
    j = "left" if rot == 0 else "right"
    return '  (label "%s" (at %.2f %.2f %d) (fields_autoplaced yes)\n    (effects (font (size 1.27 1.27)) (justify %s))\n    (uuid "%s")\n  )' % (n, x, y, rot, j, U())

def glbl(n, x, y, rot=0, shape="bidirectional"):
    j = "left" if rot == 0 else "right"
    return ('  (global_label "%s" (shape %s) (at %.2f %.2f %d)\n'
            '    (fields_autoplaced yes)\n    (effects (font (size 1.27 1.27)) (justify %s))\n    (uuid "%s")\n'
            '    (property "Intersheets" "" (at 0 0 0) (effects (font (size 1.27 1.27)) hide))\n  )' % (n, shape, x, y, rot, j, U()))

def txt(t, x, y, sz=2.54):
    return '  (text "%s" (exclude_from_sim no) (at %.2f %.2f)\n    (effects (font (size %.2f %.2f)) (justify left))\n    (uuid "%s")\n  )' % (t, x, y, sz, sz, U())

def hdr(title, uuid, paper="A4"):
    return ('(kicad_sch\n  (version 20231120)\n  (generator "petfilter_schgen")\n  (generator_version "1.0")\n'
            '  (uuid "%s")\n  (paper "%s")\n  (title_block (title "%s")(date "2026-02-15")(rev "0.1")(company "PetFilter"))' % (uuid, paper, title))

def ftr(path):
    return '  (sheet_instances\n    (path "%s" (page "1"))\n  )\n)' % path

# ---- Lib symbol helpers ----
def lsp(n):
    gnd = n == "GND"; flag = n == "PWR_FLAG"
    if gnd:
        sh = '(polyline (pts (xy 0 0)(xy 0 -1.27)(xy 1.27 -1.27)(xy 0 -2.54)(xy -1.27 -1.27)(xy 0 -1.27)) (stroke (width 0)(type default))(fill (type none)))'
        pr2, pl = 270, 0
    elif flag:
        sh = '(polyline (pts (xy 0 0)(xy 0 1.27)(xy -1.016 1.905)(xy 0 2.54)(xy 1.016 1.905)(xy 0 1.27)) (stroke (width 0)(type default))(fill (type none)))'
        pr2, pl = 90, 0
    else:
        sh = '(polyline (pts (xy -0.762 1.27)(xy 0 2.54)(xy 0.762 1.27)) (stroke (width 0)(type default))(fill (type none)))'
        pr2, pl = 90, 1.27
    vy = -3.81 if gnd else 3.81
    return ('    (symbol "power:%s" (power)(pin_names (offset 0))(exclude_from_sim no)(in_bom no)(on_board no)\n'
            '      (property "Reference" "#PWR" (at 0 %.2f 0)(effects (font (size 1.27 1.27)) hide))\n'
            '      (property "Value" "%s" (at 0 %.2f 0)(effects (font (size 1.27 1.27))))\n'
            '      (property "Footprint" "" (at 0 0 0)(effects (font (size 1.27 1.27)) hide))\n'
            '      (property "Datasheet" "" (at 0 0 0)(effects (font (size 1.27 1.27)) hide))\n'
            '      (symbol "%s_0_1" %s)\n'
            '      (symbol "%s_1_1" (pin power_in line (at 0 0 %d)(length %.2f)'
            '(name "%s" (effects (font (size 1.27 1.27))))(number "1" (effects (font (size 1.27 1.27)))))))' % (
                n, vy, n, vy, n, sh, n, pr2, pl, n))

def ls2(n):
    p = n[0].upper()
    return ('    (symbol "Device:%s" (pin_names (offset 0) hide)(exclude_from_sim no)(in_bom yes)(on_board yes)\n'
            '      (property "Reference" "%s" (at 2.54 0 90)(effects (font (size 1.27 1.27))))\n'
            '      (property "Value" "%s" (at -2.54 0 90)(effects (font (size 1.27 1.27))))\n'
            '      (property "Footprint" "" (at 0 0 0)(effects (font (size 1.27 1.27)) hide))\n'
            '      (property "Datasheet" "" (at 0 0 0)(effects (font (size 1.27 1.27)) hide))\n'
            '      (symbol "%s_0_1" (rectangle (start -1.016 2.54)(end 1.016 -2.54)(stroke (width 0.254)(type default))(fill (type none))))\n'
            '      (symbol "%s_1_1"\n'
            '        (pin passive line (at 0 3.81 270)(length 1.27)(name "~" (effects (font (size 1.27 1.27))))(number "1" (effects (font (size 1.27 1.27)))))\n'
            '        (pin passive line (at 0 -3.81 90)(length 1.27)(name "~" (effects (font (size 1.27 1.27))))(number "2" (effects (font (size 1.27 1.27)))))))' % (n, p, n, n, n))

def lsd(n):
    return ('    (symbol "Device:%s" (pin_names (offset 0) hide)(exclude_from_sim no)(in_bom yes)(on_board yes)\n'
            '      (property "Reference" "D" (at 2.54 0 90)(effects (font (size 1.27 1.27))))\n'
            '      (property "Value" "%s" (at -2.54 0 90)(effects (font (size 1.27 1.27))))\n'
            '      (property "Footprint" "" (at 0 0 0)(effects (font (size 1.27 1.27)) hide))\n'
            '      (property "Datasheet" "" (at 0 0 0)(effects (font (size 1.27 1.27)) hide))\n'
            '      (symbol "%s_0_1"\n'
            '        (polyline (pts (xy -1.27 1.27)(xy -1.27 -1.27)(xy 1.27 0)(xy -1.27 1.27))(stroke (width 0.254)(type default))(fill (type none)))\n'
            '        (polyline (pts (xy 1.27 1.27)(xy 1.27 -1.27))(stroke (width 0.254)(type default))(fill (type none))))\n'
            '      (symbol "%s_1_1"\n'
            '        (pin passive line (at 0 3.81 270)(length 2.54)(name "A" (effects (font (size 1.27 1.27))))(number "1" (effects (font (size 1.27 1.27)))))\n'
            '        (pin passive line (at 0 -3.81 90)(length 2.54)(name "K" (effects (font (size 1.27 1.27))))(number "2" (effects (font (size 1.27 1.27)))))))' % (n, n, n, n))

def lsi(name, pins, bw=10.16, bh=None):
    if bh is None:
        lc = sum(1 for p in pins if p[3] == 'left')
        rc = sum(1 for p in pins if p[3] == 'right')
        bh = max(max(lc, rc) * 2.54 + 2.54, 7.62)
    hw, hh = bw / 2, bh / 2
    pl = []
    for pn, pnum, pt, side, off in pins:
        if side == 'left': px, py, pr2 = -hw - 2.54, off, 0
        elif side == 'right': px, py, pr2 = hw + 2.54, off, 180
        elif side == 'top': px, py, pr2 = off, hh + 2.54, 270
        else: px, py, pr2 = off, -hh - 2.54, 90
        pl.append('        (pin %s line (at %.2f %.2f %d)(length 2.54)(name "%s" (effects (font (size 1.27 1.27))))(number "%s" (effects (font (size 1.27 1.27)))))' % (pt, px, py, pr2, pn, pnum))
    sn = name.split(":")[-1]
    return ('    (symbol "%s" (pin_names (offset 1.016))(exclude_from_sim no)(in_bom yes)(on_board yes)\n'
            '      (property "Reference" "U" (at 0 %.2f 0)(effects (font (size 1.27 1.27))))\n'
            '      (property "Value" "%s" (at 0 %.2f 0)(effects (font (size 1.27 1.27))))\n'
            '      (property "Footprint" "" (at 0 0 0)(effects (font (size 1.27 1.27)) hide))\n'
            '      (property "Datasheet" "" (at 0 0 0)(effects (font (size 1.27 1.27)) hide))\n'
            '      (symbol "%s_0_1" (rectangle (start %.2f %.2f)(end %.2f %.2f)(stroke (width 0.254)(type default))(fill (type background))))\n'
            '      (symbol "%s_1_1"\n' % (name, hh + 2.54, sn, -hh - 2.54, sn, -hw, hh, hw, -hh, sn)
            + "\n".join(pl) + '))')

def lsc(n):
    hh = max(n * 2.54 + 2.54, 5.08) / 2
    ps = []
    for i in range(n):
        y = hh - 2.54 - i * 2.54
        ps.append('        (pin passive line (at 5.08 %.2f 180)(length 2.54)(name "Pin_%d" (effects (font (size 1.27 1.27))))(number "%d" (effects (font (size 1.27 1.27)))))' % (y, i+1, i+1))
    return ('    (symbol "Connector_Generic:Conn_01x%02d_Pin" (pin_names (offset 1.016))(exclude_from_sim no)(in_bom yes)(on_board yes)\n'
            '      (property "Reference" "J" (at 0 %.2f 0)(effects (font (size 1.27 1.27))))\n'
            '      (property "Value" "Conn_01x%02d" (at 0 %.2f 0)(effects (font (size 1.27 1.27))))\n'
            '      (property "Footprint" "" (at 0 0 0)(effects (font (size 1.27 1.27)) hide))\n'
            '      (property "Datasheet" "" (at 0 0 0)(effects (font (size 1.27 1.27)) hide))\n'
            '      (symbol "Conn_01x%02d_Pin_0_1" (rectangle (start -1.27 %.2f)(end 1.27 %.2f)(stroke (width 0.254)(type default))(fill (type background))))\n'
            '      (symbol "Conn_01x%02d_Pin_1_1"\n' % (n, hh+2.54, n, -hh-2.54, n, hh, -hh, n)
            + "\n".join(ps) + '))')

def lssw():
    return ('    (symbol "Switch:SW_Push" (pin_names (offset 1.016) hide)(exclude_from_sim no)(in_bom yes)(on_board yes)\n'
            '      (property "Reference" "SW" (at 2.54 2.54 0)(effects (font (size 1.27 1.27))))\n'
            '      (property "Value" "SW_Push" (at 0 -2.54 0)(effects (font (size 1.27 1.27))))\n'
            '      (property "Footprint" "" (at 0 0 0)(effects (font (size 1.27 1.27)) hide))\n'
            '      (property "Datasheet" "" (at 0 0 0)(effects (font (size 1.27 1.27)) hide))\n'
            '      (symbol "SW_Push_0_1" (polyline (pts (xy -2.54 0)(xy 2.54 0))(stroke (width 0)(type default))(fill (type none))))\n'
            '      (symbol "SW_Push_1_1"\n'
            '        (pin passive line (at -5.08 0 0)(length 2.54)(name "1" (effects (font (size 1.27 1.27))))(number "1" (effects (font (size 1.27 1.27)))))\n'
            '        (pin passive line (at 5.08 0 180)(length 2.54)(name "2" (effects (font (size 1.27 1.27))))(number "2" (effects (font (size 1.27 1.27)))))))')

def lsled():
    return ('    (symbol "LED:WS2812B" (pin_names (offset 1.016))(exclude_from_sim no)(in_bom yes)(on_board yes)\n'
            '      (property "Reference" "D" (at 0 8.89 0)(effects (font (size 1.27 1.27))))\n'
            '      (property "Value" "WS2812B" (at 0 -8.89 0)(effects (font (size 1.27 1.27))))\n'
            '      (property "Footprint" "" (at 0 0 0)(effects (font (size 1.27 1.27)) hide))\n'
            '      (property "Datasheet" "" (at 0 0 0)(effects (font (size 1.27 1.27)) hide))\n'
            '      (symbol "WS2812B_0_1" (rectangle (start -5.08 7.62)(end 5.08 -7.62)(stroke (width 0.254)(type default))(fill (type background))))\n'
            '      (symbol "WS2812B_1_1"\n'
            '        (pin power_in line (at 0 10.16 270)(length 2.54)(name "VDD" (effects (font (size 1.27 1.27))))(number "1" (effects (font (size 1.27 1.27)))))\n'
            '        (pin output line (at 7.62 0 180)(length 2.54)(name "DO" (effects (font (size 1.27 1.27))))(number "2" (effects (font (size 1.27 1.27)))))\n'
            '        (pin power_in line (at 0 -10.16 90)(length 2.54)(name "GND" (effects (font (size 1.27 1.27))))(number "3" (effects (font (size 1.27 1.27)))))\n'
            '        (pin input line (at -7.62 0 0)(length 2.54)(name "DI" (effects (font (size 1.27 1.27))))(number "4" (effects (font (size 1.27 1.27)))))))')

def lsmosfet():
    return ('    (symbol "Transistor_FET:AO3400A" (pin_names (offset 1.016))(exclude_from_sim no)(in_bom yes)(on_board yes)\n'
            '      (property "Reference" "Q" (at 5.08 1.27 0)(effects (font (size 1.27 1.27)) (justify left)))\n'
            '      (property "Value" "AO3400A" (at 5.08 -1.27 0)(effects (font (size 1.27 1.27)) (justify left)))\n'
            '      (property "Footprint" "" (at 0 0 0)(effects (font (size 1.27 1.27)) hide))\n'
            '      (property "Datasheet" "" (at 0 0 0)(effects (font (size 1.27 1.27)) hide))\n'
            '      (symbol "AO3400A_0_1"\n'
            '        (polyline (pts (xy 0.254 0)(xy -2.54 0))(stroke (width 0)(type default))(fill (type none)))\n'
            '        (polyline (pts (xy 0.254 1.905)(xy 0.254 -1.905))(stroke (width 0.254)(type default))(fill (type none)))\n'
            '        (polyline (pts (xy 0.762 -1.27)(xy 0.762 -2.286))(stroke (width 0.254)(type default))(fill (type none)))\n'
            '        (polyline (pts (xy 0.762 0.508)(xy 0.762 -0.508))(stroke (width 0.254)(type default))(fill (type none)))\n'
            '        (polyline (pts (xy 0.762 2.286)(xy 0.762 1.27))(stroke (width 0.254)(type default))(fill (type none)))\n'
            '        (polyline (pts (xy 2.54 2.54)(xy 2.54 1.778)(xy 0.762 1.778))(stroke (width 0)(type default))(fill (type none)))\n'
            '        (polyline (pts (xy 2.54 -2.54)(xy 2.54 0)(xy 0.762 0))(stroke (width 0)(type default))(fill (type none))))\n'
            '      (symbol "AO3400A_1_1"\n'
            '        (pin input line (at -5.08 0 0)(length 2.54)(name "G" (effects (font (size 1.27 1.27))))(number "1" (effects (font (size 1.27 1.27)))))\n'
            '        (pin passive line (at 2.54 -5.08 90)(length 2.54)(name "S" (effects (font (size 1.27 1.27))))(number "2" (effects (font (size 1.27 1.27)))))\n'
            '        (pin passive line (at 2.54 5.08 270)(length 2.54)(name "D" (effects (font (size 1.27 1.27))))(number "3" (effects (font (size 1.27 1.27)))))))')

# =====================================================================
# ROOT SHEET
# =====================================================================
def gen_root():
    e = []
    e.append(txt("PetFilter - Main Board\\nVenturi Water-Air Pet Odor Scrubber\\nESP32-S3 + Pump + UVC + Sensors", 30, 25, 3))
    for nm, fn, su, sx, sy, sw, sh in [
        ("Power Distribution", "power.kicad_sch", SU["power"], 30, 65, 55, 25),
        ("ESP32-S3 MCU", "mcu.kicad_sch", SU["mcu"], 100, 65, 55, 25),
        ("Pump Motor Driver", "motor_driver.kicad_sch", SU["motor"], 170, 65, 55, 25),
        ("UVC LED Driver", "uvc_driver.kicad_sch", SU["uvc"], 30, 110, 55, 25),
        ("Sensor Interfaces", "sensors.kicad_sch", SU["sensors"], 100, 110, 55, 25)]:
        e.append(
            '  (sheet (at %d %d) (size %d %d)\n    (stroke (width 0.2)(type default))\n    (fill (color 255 255 255 0.0))\n    (uuid "%s")\n    (property "Sheetname" "%s" (at %d %d 0)(effects (font (size 1.27 1.27))(justify left bottom)))\n    (property "Sheetfile" "%s" (at %d %d 0)(effects (font (size 1.27 1.27))(justify left top))))' %
            (sx, sy, sw, sh, su, nm, sx, sy-2, fn, sx, sy+sh+2))
    si = "\n".join(['    (path "/%s" (page "1"))' % R] +
                   ['    (path "/%s/%s" (page "%d"))' % (R, SU[k], i+2) for i, k in enumerate(["power","mcu","motor","uvc","sensors"])])
    return hdr("PetFilter - Main Board", R, "A3") + "\n  (lib_symbols)\n" + "\n".join(e) + "\n  (sheet_instances\n" + si + "\n  )\n)"

# =====================================================================
# POWER SHEET
# =====================================================================
def gen_power():
    sp = "/%s/%s" % (R, SU["power"])
    _p[0] = 0
    mp = [("IN","2","power_in","top",-2.54),("BST","7","passive","top",2.54),("GND","3","power_in","bottom",0),
          ("EN","5","input","left",5.08),("FREQ","8","input","left",0),("FB","4","input","left",-5.08),
          ("SW","1","output","right",5.08),("VCC","6","power_out","right",-2.54)]
    ams = [("GND","1","power_in","bottom",0),("VOUT","2","power_out","right",0),("VIN","3","power_in","left",0)]
    ls = "\n".join([lsp("+12V"),lsp("+5V"),lsp("+3V3"),lsp("GND"),lsp("PWR_FLAG"),
                    ls2("R"),ls2("C"),ls2("CP"),ls2("L"),ls2("Fuse"),lsd("D_TVS"),lsd("D_Schottky"),
                    lsi("petfilter:MP1584EN",mp,12.7,15.24),lsi("Regulator_Linear:AMS1117-3.3",ams,10.16,7.62),lsc(2)])
    e = []
    e.append(txt("12V DC Input + Protection",25,20,3))
    # DC Jack
    e.append(sym("Connector_Generic:Conn_01x02_Pin","J1","DC_Jack","Connector_BarrelJack:BarrelJack_Horizontal",35,50,np=2,sp=sp))
    e.append(W(40.08,51.27,48,51.27)); e.append(lbl("+12V_R",48,51.27))
    e.append(pwr("GND",PR(),42,48.73,0,sp)); e.append(W(40.08,48.73,42,48.73))
    # Fuse
    e.append(sym("Device:Fuse","F1","5A","Fuse:Fuseholder_Blade_Mini_Keystone_3557",60,51.27,rot=90,np=2,sp=sp))
    e.append(lbl("+12V_R",56.19,51.27)); e.append(lbl("+12V_F",63.81,51.27))
    # TVS + input caps
    e.append(sym("Device:D_TVS","D1","SMBJ15A","Diode_SMD:D_SMB",72,58,np=2,sp=sp))
    e.append(lbl("+12V_F",72,54.19)); e.append(pwr("GND",PR(),72,64,0,sp)); e.append(W(72,61.81,72,64))
    e.append(sym("Device:CP","C1","100u/25V","Capacitor_THT:CP_Radial_D8.0mm_P3.50mm",80,58,np=2,sp=sp))
    e.append(lbl("+12V_F",80,54.19)); e.append(pwr("GND",PR(),80,64,0,sp)); e.append(W(80,61.81,80,64))
    e.append(sym("Device:C","C2","100n","Capacitor_SMD:C_0603_1608Metric",87,58,np=2,sp=sp))
    e.append(lbl("+12V_F",87,54.19)); e.append(pwr("GND",PR(),87,64,0,sp)); e.append(W(87,61.81,87,64))
    # Power symbols
    e.append(pwr("+12V",PR(),76,45,0,sp)); e.append(W(76,45,76,51.27)); e.append(lbl("+12V_F",76,51.27))
    e.append(pwr("PWR_FLAG",PR(),73,45,0,sp)); e.append(W(73,45,73,51.27)); e.append(lbl("+12V_F",73,51.27))
    e.append(pwr("PWR_FLAG",PR(),90,64,180,sp)); e.append(pwr("GND",PR(),90,64,0,sp))
    # MP1584EN buck
    e.append(txt("MP1584EN: 12V->5V Buck",25,78,2.54))
    ux,uy=130,105
    e.append(sym("petfilter:MP1584EN","U1","MP1584EN","Package_SO:SOIC-8_3.9x4.9mm_P1.27mm",ux,uy,np=8,sp=sp))
    e.append(lbl("+12V_F",ux-2.54,uy-19.24)); e.append(W(ux-2.54,uy-19.24,ux-2.54,uy-15.24))
    e.append(pwr("GND",PR(),ux,uy+20,0,sp)); e.append(W(ux,uy+15.24,ux,uy+20))
    e.append(lbl("SW_N",ux+15.7,uy+5.08)); e.append(W(ux+12.7,uy+5.08,ux+15.7,uy+5.08))
    e.append(lbl("BST_N",ux+2.54,uy-19.24)); e.append(W(ux+2.54,uy-19.24,ux+2.54,uy-15.24))
    e.append(lbl("FB_N",ux-16.7,uy-5.08)); e.append(W(ux-12.7,uy-5.08,ux-16.7,uy-5.08))
    e.append(lbl("+12V_F",ux-16.7,uy+5.08)); e.append(W(ux-12.7,uy+5.08,ux-16.7,uy+5.08))
    e.append(lbl("VCC_N",ux+15.7,uy-2.54)); e.append(W(ux+12.7,uy-2.54,ux+15.7,uy-2.54))
    # Input caps for buck
    for cx2,cv,cr in [(108,"22u/25V","C3"),(102,"100n","C4")]:
        fp2 = "Capacitor_SMD:C_0805_2012Metric" if "u" in cv else "Capacitor_SMD:C_0603_1608Metric"
        e.append(sym("Device:C",cr,cv,fp2,cx2,98,np=2,sp=sp))
        e.append(lbl("+12V_F",cx2,94.19)); e.append(pwr("GND",PR(),cx2,104,0,sp)); e.append(W(cx2,101.81,cx2,104))
    # BST cap, VCC cap
    e.append(sym("Device:C","C5","100n","Capacitor_SMD:C_0603_1608Metric",150,92,np=2,sp=sp))
    e.append(lbl("BST_N",150,88.19)); e.append(lbl("SW_N",150,95.81))
    e.append(sym("Device:C","C8","100n","Capacitor_SMD:C_0603_1608Metric",155,112,np=2,sp=sp))
    e.append(lbl("VCC_N",155,108.19)); e.append(pwr("GND",PR(),155,118,0,sp)); e.append(W(155,115.81,155,118))
    # Inductor + Schottky
    e.append(sym("Device:L","L1","10uH","Inductor_SMD:L_Bourns_SRN6045",160,100,rot=90,np=2,sp=sp))
    e.append(lbl("SW_N",156.19,100)); e.append(lbl("+5V_O",163.81,100))
    e.append(sym("Device:D_Schottky","D2","SS340","Diode_SMD:D_SMA",155,100,np=2,sp=sp))
    e.append(pwr("GND",PR(),155,106,0,sp)); e.append(W(155,103.81,155,106)); e.append(lbl("SW_N",155,96.19))
    # FB divider
    e.append(sym("Device:R","R1","100k","Resistor_SMD:R_0603_1608Metric",113,118,np=2,sp=sp))
    e.append(lbl("+5V_O",113,114.19)); e.append(lbl("FB_N",113,121.81))
    e.append(sym("Device:R","R2","19.1k","Resistor_SMD:R_0603_1608Metric",113,130,np=2,sp=sp))
    e.append(lbl("FB_N",113,126.19)); e.append(pwr("GND",PR(),113,136,0,sp)); e.append(W(113,133.81,113,136))
    # Output caps
    for cx2,cr in [(170,"C6"),(177,"C7")]:
        e.append(sym("Device:C",cr,"22u/10V","Capacitor_SMD:C_0805_2012Metric",cx2,107,np=2,sp=sp))
        e.append(lbl("+5V_O",cx2,103.19)); e.append(pwr("GND",PR(),cx2,113,0,sp)); e.append(W(cx2,110.81,cx2,113))
    e.append(pwr("+5V",PR(),174,94,0,sp)); e.append(W(174,94,174,100)); e.append(lbl("+5V_O",174,100))
    # AMS1117 LDO
    e.append(txt("AMS1117-3.3: 5V->3.3V LDO",25,145,2.54))
    lx2,ly2=130,165
    e.append(sym("Regulator_Linear:AMS1117-3.3","U2","AMS1117-3.3","Package_TO_SOT_SMD:SOT-223-3_TabPin2",lx2,ly2,np=3,sp=sp))
    e.append(lbl("+5V_O",lx2-14.16,ly2)); e.append(W(lx2-10.16,ly2,lx2-14.16,ly2))
    e.append(lbl("+3V3_O",lx2+14.16,ly2)); e.append(W(lx2+10.16,ly2,lx2+14.16,ly2))
    e.append(pwr("GND",PR(),lx2,ly2+12,0,sp)); e.append(W(lx2,ly2+10.16,lx2,ly2+12))
    e.append(sym("Device:C","C9","10u","Capacitor_SMD:C_0805_2012Metric",115,165,np=2,sp=sp))
    e.append(lbl("+5V_O",115,161.19)); e.append(pwr("GND",PR(),115,171,0,sp)); e.append(W(115,168.81,115,171))
    e.append(sym("Device:C","C10","22u","Capacitor_SMD:C_0805_2012Metric",148,165,np=2,sp=sp))
    e.append(lbl("+3V3_O",148,161.19)); e.append(pwr("GND",PR(),148,171,0,sp)); e.append(W(148,168.81,148,171))
    e.append(pwr("+3V3",PR(),152,157,0,sp)); e.append(W(152,157,152,161.19)); e.append(lbl("+3V3_O",152,161.19))
    return hdr("PetFilter - Power Distribution",SU["power"]) + "\n  (lib_symbols\n" + ls + "\n  )\n" + "\n".join(e) + "\n" + ftr(sp)

# =====================================================================
# MCU SHEET
# =====================================================================
def gen_mcu():
    sp = "/%s/%s" % (R, SU["mcu"])
    _p[0] = 0
    esp_pins = [
        ("3V3","2","power_in","left",17.78),("EN","3","input","left",12.7),
        ("IO4","4","bidirectional","left",7.62),("IO5","5","bidirectional","left",2.54),
        ("IO6","6","bidirectional","left",-2.54),("IO7","7","bidirectional","left",-7.62),
        ("IO15","8","bidirectional","left",-12.7),("IO16","9","bidirectional","left",-17.78),
        ("IO17","10","bidirectional","right",-17.78),("IO18","11","bidirectional","right",-12.7),
        ("IO8","12","bidirectional","right",-7.62),("IO19","13","bidirectional","right",-2.54),
        ("IO20","14","bidirectional","right",2.54),("IO3","15","bidirectional","right",7.62),
        ("IO46","16","bidirectional","right",12.7),("IO9","17","bidirectional","right",17.78),
        ("IO10","18","bidirectional","top",-10.16),("IO11","19","bidirectional","top",-5.08),
        ("IO12","20","bidirectional","top",0),("IO13","21","bidirectional","top",5.08),
        ("IO14","22","bidirectional","top",10.16),("IO21","23","bidirectional","top",15.24),
        ("IO47","24","bidirectional","bottom",-15.24),("IO48","25","bidirectional","bottom",-10.16),
        ("IO45","26","bidirectional","bottom",-5.08),("IO0","27","bidirectional","bottom",0),
        ("IO35","28","bidirectional","bottom",5.08),("IO36","29","bidirectional","bottom",10.08),
        ("IO37","30","bidirectional","bottom",15.24),("IO38","31","bidirectional","left",22.86),
        ("IO39","32","bidirectional","right",22.86),("IO40","33","bidirectional","left",27.94),
        ("IO41","34","bidirectional","right",27.94),("IO42","35","bidirectional","left",33.02),
        ("GND","1","power_in","bottom",-20.32)]
    usbc_pins = [("VBUS","1","power_out","left",5.08),("CC1","2","bidirectional","left",0),
                 ("D-","3","bidirectional","right",2.54),("D+","4","bidirectional","right",-2.54),
                 ("CC2","5","bidirectional","left",-5.08),("GND","6","power_in","bottom",0)]
    ls = "\n".join([lsp("+3V3"),lsp("+5V"),lsp("GND"),ls2("R"),ls2("C"),lssw(),lsled(),
                    lsi("RF_Module:ESP32-S3-WROOM-1",esp_pins,20.32,50.8),
                    lsi("Connector:USB_C_Receptacle_USB2.0",usbc_pins,10.16,15.24),lsc(3)])
    e = []
    e.append(txt("ESP32-S3-WROOM-1 MCU",25,15,3))
    mx,my=100,110
    e.append(sym("RF_Module:ESP32-S3-WROOM-1","U3","ESP32-S3-WROOM-1","RF_Module:ESP32-S3-WROOM-1",mx,my,np=35,sp=sp))
    # Power connections
    e.append(pwr("+3V3",PR(),mx-22.86,my-22,0,sp)); e.append(W(mx-22.86,my-22,mx-22.86,my-17.78))
    e.append(pwr("GND",PR(),mx-20.32,my+35,0,sp)); e.append(W(mx-20.32,my+25.4,mx-20.32,my+35))
    # Decoupling
    e.append(sym("Device:C","C11","100n","Capacitor_SMD:C_0603_1608Metric",mx-30,my-15,np=2,sp=sp))
    e.append(pwr("+3V3",PR(),mx-30,my-21,0,sp)); e.append(W(mx-30,my-21,mx-30,my-18.81))
    e.append(pwr("GND",PR(),mx-30,my-9,0,sp)); e.append(W(mx-30,my-11.19,mx-30,my-9))
    # EN pullup
    e.append(sym("Device:R","R3","10k","Resistor_SMD:R_0603_1608Metric",mx-30,my-5,np=2,sp=sp))
    e.append(pwr("+3V3",PR(),mx-30,my-11,0,sp)); e.append(W(mx-30,my-11,mx-30,my-8.81))
    e.append(lbl("EN",mx-30,my-1.19))
    # GPIO global labels for inter-sheet connections
    e.append(glbl("PUMP_PWM",mx+25,my-17.78,0,"output"))
    e.append(W(mx+20.32,my-17.78,mx+25,my-17.78))
    e.append(glbl("UVC_EN",mx+25,my-12.7,0,"output"))
    e.append(W(mx+20.32,my-12.7,mx+25,my-12.7))
    e.append(glbl("NH3_ADC",mx+25,my-7.62,0,"input"))
    e.append(W(mx+20.32,my-7.62,mx+25,my-7.62))
    e.append(glbl("FLOW_PULSE",mx+25,my-2.54,0,"input"))
    e.append(W(mx+20.32,my-2.54,mx+25,my-2.54))
    e.append(glbl("LEVEL1",mx+25,my+2.54,0,"input"))
    e.append(W(mx+20.32,my+2.54,mx+25,my+2.54))
    e.append(glbl("LEVEL2",mx+25,my+7.62,0,"input"))
    e.append(W(mx+20.32,my+7.62,mx+25,my+7.62))
    e.append(glbl("TEMP_ADC",mx+25,my+12.7,0,"input"))
    e.append(W(mx+20.32,my+12.7,mx+25,my+12.7))
    e.append(glbl("INTERLOCK",mx+25,my+17.78,0,"input"))
    e.append(W(mx+20.32,my+17.78,mx+25,my+17.78))
    e.append(glbl("MQ_HEATER",mx-25,my-7.62,180,"output"))
    e.append(W(mx-20.32,my-7.62,mx-25,my-7.62))
    # USB-C
    e.append(txt("USB-C (Programming/Debug)",25,160,2.54))
    ux,uy=60,185
    e.append(sym("Connector:USB_C_Receptacle_USB2.0","J2","USB_C","Connector_USB:USB_C_Receptacle_GCT_USB4105",ux,uy,np=6,sp=sp))
    e.append(pwr("+5V",PR(),ux-14,uy+5.08,0,sp)); e.append(W(ux-10.16,uy+5.08,ux-14,uy+5.08))
    e.append(pwr("GND",PR(),ux,uy+20,0,sp)); e.append(W(ux,uy+15.24,ux,uy+20))
    # CC resistors
    e.append(sym("Device:R","R4","5.1k","Resistor_SMD:R_0603_1608Metric",ux-15,uy,np=2,sp=sp))
    e.append(W(ux-10.16,uy,ux-15,uy+3.81)); e.append(pwr("GND",PR(),ux-15,uy+6,0,sp)); e.append(W(ux-15,uy+3.81,ux-15,uy+6))
    e.append(sym("Device:R","R5","5.1k","Resistor_SMD:R_0603_1608Metric",ux-15,uy-5.08,np=2,sp=sp))
    e.append(W(ux-10.16,uy-5.08,ux-15,uy-5.08+3.81)); e.append(pwr("GND",PR(),ux-15,uy-5.08+6,0,sp))
    e.append(lbl("USB_DP",ux+14,uy+2.54)); e.append(W(ux+10.16,uy+2.54,ux+14,uy+2.54))
    e.append(lbl("USB_DM",ux+14,uy-2.54)); e.append(W(ux+10.16,uy-2.54,ux+14,uy-2.54))
    # Buttons
    e.append(txt("Boot/Reset Buttons",130,160,2.54))
    e.append(sym("Switch:SW_Push","SW1","RESET","Switch_SMD:SW_Push_1P1T_NO_6x6mm_H9.5mm",160,175,np=2,sp=sp))
    e.append(lbl("EN",154.92,175)); e.append(pwr("GND",PR(),165.08,175,0,sp))
    e.append(sym("Switch:SW_Push","SW2","BOOT","Switch_SMD:SW_Push_1P1T_NO_6x6mm_H9.5mm",160,190,np=2,sp=sp))
    e.append(lbl("IO0",154.92,190)); e.append(pwr("GND",PR(),165.08,190,0,sp))
    # WS2812B
    e.append(txt("Status LED (WS2812B)",130,200,2.54))
    e.append(sym("LED:WS2812B","D3","WS2812B","LED_SMD:LED_WS2812B_PLCC4_5.0x5.0mm_P3.2mm",170,215,np=4,sp=sp))
    e.append(pwr("+5V",PR(),170,202,0,sp)); e.append(W(170,202,170,204.84))
    e.append(pwr("GND",PR(),170,228,0,sp)); e.append(W(170,225.16,170,228))
    e.append(lbl("LED_DI",162.38,215)); e.append(glbl("LED_DATA",mx-25,my-12.7,180,"output"))
    e.append(W(mx-20.32,my-12.7,mx-25,my-12.7))
    return hdr("PetFilter - ESP32-S3 MCU",SU["mcu"]) + "\n  (lib_symbols\n" + ls + "\n  )\n" + "\n".join(e) + "\n" + ftr(sp)

# =====================================================================
# MOTOR DRIVER SHEET
# =====================================================================
def gen_motor():
    sp = "/%s/%s" % (R, SU["motor"])
    _p[0] = 0
    drv_pins = [("VM","1","power_in","top",0),("OUT1","2","output","right",2.54),
                ("OUT2","3","output","right",-2.54),("GND","4","power_in","bottom",0),
                ("ISEN","5","input","left",-5.08),("IN2","6","input","left",0),
                ("IN1","7","input","left",5.08),("VCC","8","power_in","top",5.08)]
    ls = "\n".join([lsp("+12V"),lsp("+3V3"),lsp("GND"),ls2("R"),ls2("C"),
                    lsi("petfilter:DRV8871",drv_pins,10.16,15.24),lsc(2)])
    e = []
    e.append(txt("DRV8871 Pump Motor Driver",25,20,3))
    dx,dy=100,70
    e.append(sym("petfilter:DRV8871","U4","DRV8871","Package_SO:HTSSOP-8-1EP_4.4x3mm_P0.65mm",dx,dy,np=8,sp=sp))
    e.append(pwr("+12V",PR(),dx,dy-20,0,sp)); e.append(W(dx,dy-15.24,dx,dy-20))
    e.append(pwr("GND",PR(),dx,dy+20,0,sp)); e.append(W(dx,dy+15.24,dx,dy+20))
    e.append(pwr("+3V3",PR(),dx+5.08,dy-20,0,sp)); e.append(W(dx+5.08,dy-15.24,dx+5.08,dy-20))
    # Decoupling
    e.append(sym("Device:C","C13","100n","Capacitor_SMD:C_0603_1608Metric",dx-12,dy-10,np=2,sp=sp))
    e.append(pwr("+12V",PR(),dx-12,dy-16,0,sp)); e.append(W(dx-12,dy-16,dx-12,dy-13.81))
    e.append(pwr("GND",PR(),dx-12,dy-4,0,sp)); e.append(W(dx-12,dy-6.19,dx-12,dy-4))
    # Control inputs
    e.append(glbl("PUMP_PWM",dx-18,dy+5.08,180,"input"))
    e.append(W(dx-12.7,dy+5.08,dx-18,dy+5.08))
    e.append(pwr("GND",PR(),dx-18,dy,0,sp)); e.append(W(dx-12.7,dy,dx-18,dy))
    # Current sense
    e.append(sym("Device:R","R6","0.2","Resistor_SMD:R_2512_6332Metric",dx-18,dy-5.08,np=2,sp=sp))
    e.append(W(dx-12.7,dy-5.08,dx-18,dy-5.08+3.81)); e.append(pwr("GND",PR(),dx-18,dy-5.08+6,0,sp))
    # Pump connector
    e.append(sym("Connector_Generic:Conn_01x02_Pin","J3","PUMP","Connector_Molex:Molex_Micro-Fit_3.0_43045-0200",dx+20,dy,np=2,sp=sp))
    e.append(W(dx+12.7,dy+2.54,dx+20,dy+2.54)); e.append(W(dx+12.7,dy-2.54,dx+20,dy-2.54))
    return hdr("PetFilter - Pump Motor Driver",SU["motor"]) + "\n  (lib_symbols\n" + ls + "\n  )\n" + "\n".join(e) + "\n" + ftr(sp)

# =====================================================================
# UVC DRIVER SHEET
# =====================================================================
def gen_uvc():
    sp = "/%s/%s" % (R, SU["uvc"])
    _p[0] = 0
    pt_pins = [("VIN","1","power_in","left",5.08),("SW","2","output","top",0),
               ("DIM","3","input","left",-5.08),("CSN","4","input","bottom",0),
               ("GND","5","power_in","bottom",-5.08)]
    ls = "\n".join([lsp("+12V"),lsp("+3V3"),lsp("GND"),ls2("R"),ls2("C"),ls2("L"),lsd("D_Schottky"),
                    lsi("petfilter:PT4115",pt_pins,10.16,15.24),lsc(2),lsc(3)])
    e = []
    e.append(txt("PT4115 UVC LED Driver",25,20,3))
    px,py=100,70
    e.append(sym("petfilter:PT4115","U5","PT4115","Package_SO:SOIC-8_3.9x4.9mm_P1.27mm",px,py,np=5,sp=sp))
    e.append(pwr("+12V",PR(),px-14,py+5.08,0,sp)); e.append(W(px-10.16,py+5.08,px-14,py+5.08))
    e.append(pwr("GND",PR(),px-5.08,py+20,0,sp)); e.append(W(px-5.08,py+15.24,px-5.08,py+20))
    # Inductor
    e.append(sym("Device:L","L2","47uH","Inductor_SMD:L_Bourns_SRN6045",px,py-25,np=2,sp=sp))
    e.append(W(px,py-15.24,px,py-21.19)); e.append(lbl("SW_UVC",px,py-28.81))
    # Schottky
    e.append(sym("Device:D_Schottky","D4","SS340","Diode_SMD:D_SMA",px+12,py-18,np=2,sp=sp))
    e.append(lbl("SW_UVC",px+12,py-21.81)); e.append(lbl("UVC_OUT",px+12,py-14.19))
    # Current sense
    e.append(sym("Device:R","R7","0.33","Resistor_SMD:R_2512_6332Metric",px,py+25,np=2,sp=sp))
    e.append(W(px,py+15.24,px,py+21.19)); e.append(pwr("GND",PR(),px,py+32,0,sp)); e.append(W(px,py+28.81,px,py+32))
    # DIM control
    e.append(glbl("UVC_EN",px-18,py-5.08,180,"input"))
    e.append(W(px-10.16,py-5.08,px-18,py-5.08))
    # UVC LED connector
    e.append(sym("Connector_Generic:Conn_01x02_Pin","J4","UVC_LED","Connector_Molex:Molex_Micro-Fit_3.0_43045-0200",px+30,py-10,np=2,sp=sp))
    e.append(lbl("UVC_OUT",px+25,py-10)); e.append(pwr("GND",PR(),px+30,py-5,0,sp))
    # Interlock
    e.append(txt("Interlock Switch",25,120,2.54))
    e.append(sym("Connector_Generic:Conn_01x03_Pin","J5","INTERLOCK","Connector_PinHeader_2.54mm:PinHeader_1x03_P2.54mm_Vertical",80,135,np=3,sp=sp))
    e.append(pwr("+3V3",PR(),90,130,0,sp)); e.append(glbl("INTERLOCK",90,135,0,"input"))
    e.append(pwr("GND",PR(),90,140,0,sp))
    e.append(sym("Device:R","R8","10k","Resistor_SMD:R_0603_1608Metric",95,135,np=2,sp=sp))
    e.append(pwr("+3V3",PR(),95,129,0,sp)); e.append(W(95,129,95,131.19))
    return hdr("PetFilter - UVC LED Driver",SU["uvc"]) + "\n  (lib_symbols\n" + ls + "\n  )\n" + "\n".join(e) + "\n" + ftr(sp)

# =====================================================================
# SENSORS SHEET
# =====================================================================
def gen_sensors():
    sp = "/%s/%s" % (R, SU["sensors"])
    _p[0] = 0
    ls = "\n".join([lsp("+5V"),lsp("+3V3"),lsp("GND"),ls2("R"),ls2("C"),lsc(3),lsc(4),lsmosfet()])
    e = []
    e.append(txt("Sensor Interfaces",25,15,3))
    # MQ-137
    e.append(txt("MQ-137 NH3 Sensor (Analog + MOSFET Heater)",25,35,2.54))
    e.append(sym("Connector_Generic:Conn_01x04_Pin","J6","MQ137","Connector_JST:JST_PH_B4B-PH-K_1x04_P2.00mm_Vertical",60,55,np=4,sp=sp))
    e.append(pwr("+5V",PR(),70,47,0,sp)); e.append(glbl("NH3_ADC",70,52,0,"output"))
    e.append(pwr("GND",PR(),70,57,0,sp)); e.append(lbl("MQ_HTR",70,62))
    e.append(sym("Transistor_FET:AO3400A","Q1","AO3400A","Package_TO_SOT_SMD:SOT-23",90,65,np=3,sp=sp))
    e.append(lbl("MQ_HTR",84.92,65)); e.append(pwr("+5V",PR(),92.54,57,0,sp)); e.append(pwr("GND",PR(),92.54,73,0,sp))
    e.append(W(92.54,59.92,92.54,57)); e.append(W(92.54,70.08,92.54,73))
    e.append(glbl("MQ_HEATER",80,65,180,"input")); e.append(W(80,65,84.92,65))
    e.append(sym("Device:R","R9","10k","Resistor_SMD:R_0603_1608Metric",85,75,np=2,sp=sp))
    e.append(lbl("MQ_HTR",85,71.19)); e.append(pwr("GND",PR(),85,81,0,sp)); e.append(W(85,78.81,85,81))
    e.append(sym("Device:R","R10","10k","Resistor_SMD:R_0603_1608Metric",70,70,np=2,sp=sp))
    e.append(glbl("NH3_ADC",70,66.19,0,"output")); e.append(pwr("GND",PR(),70,76,0,sp)); e.append(W(70,73.81,70,76))
    # YF-S201
    e.append(txt("YF-S201 Flow Sensor (Pulse)",25,95,2.54))
    e.append(sym("Connector_Generic:Conn_01x03_Pin","J7","FLOW","Connector_JST:JST_PH_B3B-PH-K_1x03_P2.00mm_Vertical",60,110,np=3,sp=sp))
    e.append(pwr("+5V",PR(),70,105,0,sp)); e.append(glbl("FLOW_PULSE",70,110,0,"output"))
    e.append(pwr("GND",PR(),70,115,0,sp))
    e.append(sym("Device:R","R11","10k","Resistor_SMD:R_0603_1608Metric",80,105,np=2,sp=sp))
    e.append(pwr("+3V3",PR(),80,99,0,sp)); e.append(W(80,99,80,101.19)); e.append(glbl("FLOW_PULSE",80,108.81,0,"output"))
    # XKC-Y25 x2
    e.append(txt("XKC-Y25 Water Level x2 (Digital)",25,130,2.54))
    for i,gn,jref,yoff in [(1,"LEVEL1","J8",145),(2,"LEVEL2","J9",170)]:
        e.append(sym("Connector_Generic:Conn_01x03_Pin",jref,"LEVEL%d"%i,"Connector_JST:JST_PH_B3B-PH-K_1x03_P2.00mm_Vertical",60,yoff,np=3,sp=sp))
        e.append(pwr("+5V",PR(),70,yoff-5,0,sp)); e.append(glbl(gn,70,yoff,0,"input"))
        e.append(pwr("GND",PR(),70,yoff+5,0,sp))
        e.append(sym("Device:R","R%d"%(11+i),"10k","Resistor_SMD:R_0603_1608Metric",80,yoff-2,np=2,sp=sp))
        e.append(pwr("+3V3",PR(),80,yoff-8,0,sp)); e.append(W(80,yoff-8,80,yoff-5.81)); e.append(glbl(gn,80,yoff+1.81,0,"input"))
    # NTC Thermistor
    e.append(txt("NTC Thermistor (10K, Analog)",25,195,2.54))
    e.append(sym("Connector_Generic:Conn_01x03_Pin","J10","THERM","Connector_JST:JST_PH_B3B-PH-K_1x03_P2.00mm_Vertical",60,210,np=3,sp=sp))
    e.append(pwr("+3V3",PR(),70,205,0,sp)); e.append(glbl("TEMP_ADC",70,210,0,"output"))
    e.append(pwr("GND",PR(),70,215,0,sp))
    e.append(sym("Device:R","R14","10k","Resistor_SMD:R_0603_1608Metric",80,205,np=2,sp=sp))
    e.append(pwr("+3V3",PR(),80,199,0,sp)); e.append(W(80,199,80,201.19)); e.append(glbl("TEMP_ADC",80,208.81,0,"output"))
    e.append(sym("Device:C","C12","100n","Capacitor_SMD:C_0603_1608Metric",90,210,np=2,sp=sp))
    e.append(glbl("TEMP_ADC",90,206.19,0,"output")); e.append(pwr("GND",PR(),90,216,0,sp)); e.append(W(90,213.81,90,216))
    return hdr("PetFilter - Sensor Interfaces",SU["sensors"]) + "\n  (lib_symbols\n" + ls + "\n  )\n" + "\n".join(e) + "\n" + ftr(sp)

# =====================================================================
# GENERATE ALL FILES
# =====================================================================
files = {
    "petfilter.kicad_sch": gen_root(),
    "power.kicad_sch": gen_power(),
    "mcu.kicad_sch": gen_mcu(),
    "motor_driver.kicad_sch": gen_motor(),
    "uvc_driver.kicad_sch": gen_uvc(),
    "sensors.kicad_sch": gen_sensors()
}

for fn, content in files.items():
    fp = os.path.join(outdir, fn)
    with open(fp, 'w') as f:
        f.write(content)
    print("  %s (%d lines)" % (fp, content.count('\n') + 1))

print("\nGenerated %d schematic files in %s" % (len(files), outdir))
