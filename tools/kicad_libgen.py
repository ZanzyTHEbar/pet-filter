#!/usr/bin/env python3
"""
KiCad Library Generator Tool
=============================

Generates KiCad .kicad_sym symbol and .kicad_mod footprint files
from structured component specifications.

This is the PetFilter project's implementation of the "PDF to Library"
workflow described by Adafruit/Limor Fried -- using AI + plain-text
KiCad S-expressions to skip the footprint editor entirely.

Usage:
    python kicad_libgen.py --help
    python kicad_libgen.py generate-symbol --name PT4115 --spec spec.json
    python kicad_libgen.py generate-footprint --name SOT-89-5 --spec spec.json

The key insight: KiCad's .kicad_sym and .kicad_mod files are plain-text
S-expressions. Every pad coordinate, every pin position, every courtyard
dimension is just a number in a text file. AI models excel at generating
structured markup -- this is exactly what they're good at.
"""

import argparse
import json
import math
import sys
import uuid
from dataclasses import dataclass, field
from pathlib import Path
from typing import Optional


# ==============================================================
# Data Models
# ==============================================================

@dataclass
class Pin:
    """Schematic symbol pin definition."""
    number: str
    name: str
    electrical_type: str  # power_in, power_out, input, output, passive, bidirectional, tri_state, unspecified
    x: float = 0.0
    y: float = 0.0
    orientation: int = 0  # 0=right, 90=up, 180=left, 270=down
    length: float = 2.54
    shape: str = "line"  # line, inverted, clock, inverted_clock


@dataclass
class Pad:
    """Footprint pad definition."""
    number: str
    pad_type: str  # smd, thru_hole, np_thru_hole, connect
    shape: str  # circle, rect, oval, roundrect, trapezoid
    x: float = 0.0
    y: float = 0.0
    width: float = 1.0
    height: float = 1.0
    drill: Optional[float] = None
    layers: str = '"F.Cu" "F.Paste" "F.Mask"'
    rotation: float = 0.0


@dataclass
class SymbolSpec:
    """Complete symbol specification."""
    name: str
    reference: str = "U"
    description: str = ""
    datasheet: str = ""
    footprint: str = ""
    pins: list = field(default_factory=list)
    body_width: float = 10.16
    body_height: float = 15.24


@dataclass
class FootprintSpec:
    """Complete footprint specification."""
    name: str
    description: str = ""
    tags: str = ""
    attr: str = "smd"  # smd or through_hole
    pads: list = field(default_factory=list)
    body_width: float = 5.0
    body_height: float = 5.0
    courtyard_margin: float = 0.5


# ==============================================================
# UUID Generation (deterministic for reproducibility)
# ==============================================================

def gen_uuid() -> str:
    """Generate a random UUID v4 string."""
    return str(uuid.uuid4())


# ==============================================================
# Symbol Generator
# ==============================================================

def generate_symbol(spec: SymbolSpec) -> str:
    """Generate a KiCad .kicad_sym symbol definition."""
    
    half_w = spec.body_width / 2
    half_h = spec.body_height / 2
    
    pins_sexpr = ""
    for pin in spec.pins:
        pins_sexpr += f"""
      (pin {pin.electrical_type} {pin.shape}
        (at {pin.x} {pin.y} {pin.orientation})
        (length {pin.length})
        (name "{pin.name}" (effects (font (size 1.27 1.27))))
        (number "{pin.number}" (effects (font (size 1.27 1.27))))
      )"""
    
    return f"""  (symbol "{spec.name}"
    (pin_names (offset 1.016))
    (exclude_from_sim no)
    (in_bom yes)
    (on_board yes)
    (property "Reference" "{spec.reference}"
      (at 0 {half_h + 2.54} 0)
      (effects (font (size 1.27 1.27)))
    )
    (property "Value" "{spec.name}"
      (at 0 {-(half_h + 2.54)} 0)
      (effects (font (size 1.27 1.27)))
    )
    (property "Footprint" "{spec.footprint}"
      (at 0 {-(half_h + 5.08)} 0)
      (effects (font (size 1.27 1.27)) hide)
    )
    (property "Datasheet" "{spec.datasheet}"
      (at 0 {-(half_h + 7.62)} 0)
      (effects (font (size 1.27 1.27)) hide)
    )
    (property "Description" "{spec.description}"
      (at 0 {-(half_h + 10.16)} 0)
      (effects (font (size 1.27 1.27)) hide)
    )
    (symbol "{spec.name}_0_1"
      (rectangle
        (start {-half_w} {half_h})
        (end {half_w} {-half_h})
        (stroke (width 0.254) (type default))
        (fill (type background))
      )
    )
    (symbol "{spec.name}_1_1"{pins_sexpr}
    )
  )"""


def generate_symbol_library(symbols: list[SymbolSpec]) -> str:
    """Generate a complete .kicad_sym library file."""
    
    symbol_defs = "\n\n".join(generate_symbol(s) for s in symbols)
    
    return f"""(kicad_symbol_lib
  (version 20231120)
  (generator "petfilter_libgen")
  (generator_version "1.0")

{symbol_defs}
)
"""


# ==============================================================
# Footprint Generator
# ==============================================================

def generate_footprint(spec: FootprintSpec) -> str:
    """Generate a KiCad .kicad_mod footprint definition."""
    
    half_w = spec.body_width / 2
    half_h = spec.body_height / 2
    cy_margin = spec.courtyard_margin
    
    # Generate pad definitions
    pads_sexpr = ""
    for pad in spec.pads:
        drill_str = ""
        if pad.drill is not None:
            drill_str = f"\n    (drill {pad.drill})"
        
        layers = pad.layers
        if pad.pad_type == "thru_hole":
            layers = '"*.Cu" "*.Mask"'
        
        rot_str = ""
        if pad.rotation != 0:
            rot_str = f" {pad.rotation}"
        
        pads_sexpr += f"""
  (pad "{pad.number}" {pad.pad_type} {pad.shape}
    (at {pad.x} {pad.y}{rot_str})
    (size {pad.width} {pad.height}){drill_str}
    (layers {layers})
    (uuid "{gen_uuid()}")
  )"""
    
    return f"""(footprint "{spec.name}"
  (version 20231014)
  (generator "petfilter_libgen")
  (layer "F.Cu")
  (descr "{spec.description}")
  (tags "{spec.tags}")
  (attr {spec.attr})
  (fp_text reference "REF**"
    (at 0 {-(half_h + 2)})
    (layer "F.SilkS")
    (effects (font (size 1 1) (thickness 0.15)))
    (uuid "{gen_uuid()}")
  )
  (fp_text value "{spec.name}"
    (at 0 {half_h + 2})
    (layer "F.Fab")
    (effects (font (size 1 1) (thickness 0.15)))
    (uuid "{gen_uuid()}")
  )
  (fp_rect
    (start {-half_w} {-half_h})
    (end {half_w} {half_h})
    (stroke (width 0.1) (type default))
    (fill no)
    (layer "F.Fab")
    (uuid "{gen_uuid()}")
  )
  (fp_rect
    (start {-(half_w + cy_margin)} {-(half_h + cy_margin)})
    (end {half_w + cy_margin} {half_h + cy_margin})
    (stroke (width 0.05) (type default))
    (fill no)
    (layer "F.CrtYd")
    (uuid "{gen_uuid()}")
  ){pads_sexpr}
)
"""


# ==============================================================
# Circular Pad Array (for sensors like MQ-137)
# ==============================================================

def circular_pad_array(
    num_pins: int,
    radius: float,
    pad_type: str = "thru_hole",
    pad_shape: str = "circle",
    pad_width: float = 1.4,
    pad_height: float = 1.4,
    drill: Optional[float] = 0.8,
    start_angle: float = 0.0,
) -> list[Pad]:
    """Generate pads arranged in a circle (for TO-5, MQ sensors, etc.)."""
    pads = []
    for i in range(num_pins):
        angle_rad = math.radians(start_angle + (360.0 / num_pins) * i)
        x = round(radius * math.cos(angle_rad), 3)
        y = round(-radius * math.sin(angle_rad), 3)  # KiCad Y is inverted
        pads.append(Pad(
            number=str(i + 1),
            pad_type=pad_type,
            shape=pad_shape,
            x=x, y=y,
            width=pad_width, height=pad_height,
            drill=drill,
        ))
    return pads


# ==============================================================
# Linear Pad Array (for SOIC, SOT, QFP, etc.)
# ==============================================================

def dual_row_smd_pads(
    num_pins: int,
    pitch: float,
    pad_width: float,
    pad_height: float,
    row_spacing: float,
) -> list[Pad]:
    """Generate SMD pads in two rows (SOIC, TSSOP, etc.)."""
    pads = []
    pins_per_side = num_pins // 2
    
    # Calculate starting X offset to center the row
    start_x = -pitch * (pins_per_side - 1) / 2
    
    # Bottom row (pins 1 to N/2)
    for i in range(pins_per_side):
        pads.append(Pad(
            number=str(i + 1),
            pad_type="smd",
            shape="rect",
            x=round(start_x + pitch * i, 3),
            y=round(row_spacing / 2, 3),
            width=pad_width,
            height=pad_height,
        ))
    
    # Top row (pins N/2+1 to N, reversed)
    for i in range(pins_per_side):
        pads.append(Pad(
            number=str(num_pins - i),
            pad_type="smd",
            shape="rect",
            x=round(start_x + pitch * i, 3),
            y=round(-row_spacing / 2, 3),
            width=pad_width,
            height=pad_height,
        ))
    
    return pads


# ==============================================================
# QFN Exposed Pad with DFM Paste Segmentation
# ==============================================================

def qfn_exposed_pad_with_paste_grid(
    pad_width: float,
    pad_height: float,
    grid_cols: int = 3,
    grid_rows: int = 3,
    paste_coverage: float = 0.40,
    pad_number: str = "EP",
) -> list[Pad]:
    """
    Generate a QFN exposed pad with segmented paste mask.
    
    This implements the DFM best practice described in the Adafruit article:
    instead of a solid paste mask (which causes flux outgassing and chip float),
    we create a windowed grid achieving ~40% paste coverage.
    
    The main pad has paste disabled, and individual small paste rectangles
    are added on the F.Paste layer.
    """
    pads = []
    
    # Main copper pad (no paste)
    pads.append(Pad(
        number=pad_number,
        pad_type="smd",
        shape="rect",
        x=0, y=0,
        width=pad_width,
        height=pad_height,
        layers='"F.Cu" "F.Mask"',  # No F.Paste!
    ))
    
    # Calculate paste window dimensions for target coverage
    total_paste_area = pad_width * pad_height * paste_coverage
    window_w = round(math.sqrt(total_paste_area / (grid_cols * grid_rows)) * (pad_width / pad_height) ** 0.5, 3)
    window_h = round(total_paste_area / (grid_cols * grid_rows * window_w), 3)
    
    # Paste grid spacing
    x_spacing = pad_width / (grid_cols + 1)
    y_spacing = pad_height / (grid_rows + 1)
    
    for row in range(grid_rows):
        for col in range(grid_cols):
            px = round(-pad_width / 2 + x_spacing * (col + 1), 3)
            py = round(-pad_height / 2 + y_spacing * (row + 1), 3)
            pads.append(Pad(
                number=pad_number,
                pad_type="smd",
                shape="rect",
                x=px, y=py,
                width=window_w,
                height=window_h,
                layers='"F.Paste"',  # Paste only
            ))
    
    return pads


# ==============================================================
# CLI Interface
# ==============================================================

def main():
    parser = argparse.ArgumentParser(
        description="KiCad Library Generator - PDF to .kicad_sym/.kicad_mod"
    )
    parser.add_argument(
        "command",
        choices=["demo", "generate-symbol", "generate-footprint"],
        help="Command to run"
    )
    parser.add_argument("--output", "-o", type=Path, help="Output file path")
    parser.add_argument("--name", type=str, help="Component name")
    
    args = parser.parse_args()
    
    if args.command == "demo":
        print("=== KiCad Library Generator Demo ===")
        print()
        print("Generating example SOIC-8 footprint (MP1584EN)...")
        
        pads = dual_row_smd_pads(
            num_pins=8,
            pitch=1.27,
            pad_width=0.6,
            pad_height=1.5,
            row_spacing=5.4,
        )
        
        spec = FootprintSpec(
            name="SOIC-8_MP1584EN",
            description="SOIC-8, 1.27mm pitch, for MP1584EN buck converter",
            tags="SOIC-8 MP1584EN buck converter",
            attr="smd",
            pads=pads,
            body_width=3.9,
            body_height=4.9,
        )
        
        print(generate_footprint(spec))
        
        print("\n=== Circular pad array demo (MQ-137 style) ===")
        pads = circular_pad_array(6, radius=4.5, start_angle=0)
        for p in pads:
            print(f"  Pin {p.number}: ({p.x}, {p.y})")
        
        print("\n=== QFN exposed pad with DFM paste grid ===")
        ep_pads = qfn_exposed_pad_with_paste_grid(
            pad_width=5.0,
            pad_height=5.0,
            grid_cols=3,
            grid_rows=3,
            paste_coverage=0.40,
        )
        print(f"  Main pad + {len(ep_pads)-1} paste windows for ~40% coverage")
        
        print("\nDone! Use 'generate-symbol' or 'generate-footprint' for real output.")


if __name__ == "__main__":
    main()
