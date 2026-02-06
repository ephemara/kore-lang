"""
KAIN Timeline Animation Generator
==================================
Creates an animated PNG showing the evolution of computing from 1978 to 2026.
"""

import os
import io
from PIL import Image, ImageDraw, ImageFont
from apng import APNG, PNG

# --- CONFIG ---
DESKTOP = os.path.join(os.path.expanduser("~"), "Desktop")
OUTPUT_FOLDER = os.path.join(DESKTOP, "Kain_Branding")
OUTPUT_APNG = os.path.join(OUTPUT_FOLDER, "kain_timeline.png")
OUTPUT_GIF = os.path.join(OUTPUT_FOLDER, "kain_timeline.gif")

os.makedirs(OUTPUT_FOLDER, exist_ok=True)

W, H = 600, 200
TEXT_MAIN = "KAIN"
FONT_SIZE = 90

# Timing (ms)
TYPE_SPEED = 150
BLINK_SPEED = 400
HOLD_COUNT = 5
BACKSPACE_SPEED = 70
TRANSITION_BLINKS = 3

# Era definitions
ERAS = [
    {
        "id": "TERMINAL",
        "font": ["consola.ttf", "lucon.ttf", "cour.ttf", "Courier New"],
        "color": (51, 255, 51),
        "cursor": "█",
        "effects": ["glow"],
    },
    {
        "id": "MAC_84",
        "font": ["timesbd.ttf", "georgiab.ttf", "Times New Roman Bold"],
        "color": (255, 255, 255),
        "outline": (40, 40, 40),
        "cursor": "|",
        "effects": ["outline_heavy"],
    },
    {
        "id": "WIN_XP",
        "font": ["tahomabd.ttf", "arialbd.ttf", "Tahoma Bold"],
        "color": (255, 255, 255),
        "cursor": "|",
        "effects": ["drop_shadow"],
        "shadow": (0, 78, 152)
    },
    {
        "id": "WIN_7",
        "font": ["segoeui.ttf", "calibri.ttf", "Arial"],
        "color": (255, 255, 255),
        "cursor": "|",
        "effects": ["aero_glow"],
        "glow_color": (120, 200, 255)
    },
    {
        "id": "MODERN",
        "font": ["segoeuil.ttf", "calibril.ttf", "Arial"],
        "color": (255, 255, 255),
        "cursor": "_",
        "effects": ["clean"],
    }
]

def load_best_font(names, size):
    for name in names:
        try:
            return ImageFont.truetype(name, size)
        except:
            continue
    return ImageFont.load_default()

def draw_fx_text(draw, x, y, text, font, era):
    color = era["color"]
    effects = era["effects"]
    
    # Layer 1: Background effects
    if "drop_shadow" in effects:
        draw.text((x + 4, y + 4), text, font=font, fill=era["shadow"])
        
    if "aero_glow" in effects:
        glow = era["glow_color"]
        for i in range(1, 5):
            alpha = max(10, 80 - i*15)
            draw.text((x,y), text, font=font, stroke_width=i*2, stroke_fill=(*glow, alpha))
            
    if "glow" in effects:
        draw.text((x, y), text, font=font, stroke_width=2, stroke_fill=(*color, 60))
    
    # Layer 2: Main text
    if "outline_heavy" in effects:
        draw.text((x, y), text, font=font, fill=color, stroke_width=4, stroke_fill=era["outline"])
    else:
        draw.text((x, y), text, font=font, fill=(*color, 255))

def create_frame(content, show_cursor, era_idx):
    era = ERAS[era_idx]
    img = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    
    font = load_best_font(era["font"], FONT_SIZE)
    
    # Center alignment
    full_text = TEXT_MAIN + era["cursor"]
    try:
        tw = draw.textlength(full_text, font=font)
    except:
        tw = len(full_text) * (FONT_SIZE // 2)
        
    tx = (W - tw) // 2
    ty = (H - FONT_SIZE) // 2
    
    # Draw content
    if content:
        draw_fx_text(draw, tx, ty, content, font, era)
        
    # Draw cursor
    if show_cursor:
        try:
            cur_off = draw.textlength(content, font=font) if content else 0
        except:
            cur_off = len(content) * (FONT_SIZE // 2) if content else 0
            
        cur_x = tx + cur_off + 5
        draw.text((cur_x, ty), era["cursor"], font=font, fill=(*era["color"], 255))
            
    return img

print("=" * 60)
print("KAIN TIMELINE ANIMATION GENERATOR")
print("=" * 60)

all_frames = []
all_delays = []
era_frame_counts = []

for idx, era in enumerate(ERAS):
    print(f"\n[{idx+1}/5] {era['id']}")
    frame_count_before = len(all_frames)
    
    # 1. Type in
    curr = ""
    for char in TEXT_MAIN:
        curr += char
        all_frames.append(create_frame(curr, True, idx))
        all_delays.append(TYPE_SPEED)
    print(f"  Typed: {len(TEXT_MAIN)} chars")
        
    # 2. Hold & Blink
    for _ in range(HOLD_COUNT):
        all_frames.append(create_frame(curr, False, idx))
        all_delays.append(BLINK_SPEED)
        all_frames.append(create_frame(curr, True, idx))
        all_delays.append(BLINK_SPEED)
    print(f"  Hold: {HOLD_COUNT} blinks")
        
    # 3. Backspace
    while curr:
        curr = curr[:-1]
        all_frames.append(create_frame(curr, True, idx))
        all_delays.append(BACKSPACE_SPEED)
    print(f"  Backspaced to empty")
        
    # 4. Transition Blinks (Empty)
    for _ in range(TRANSITION_BLINKS):
        all_frames.append(create_frame("", False, idx))
        all_delays.append(BLINK_SPEED)
        all_frames.append(create_frame("", True, idx))
        all_delays.append(BLINK_SPEED)
    print(f"  Transition: {TRANSITION_BLINKS} blinks")
    
    era_frames = len(all_frames) - frame_count_before
    era_frame_counts.append(era_frames)
    print(f"  Total frames for this era: {era_frames}")

print(f"\n{'=' * 60}")
print(f"TOTAL FRAMES: {len(all_frames)}")
print(f"Duration: {sum(all_delays) / 1000:.1f} seconds")
print(f"Per-era breakdown: {era_frame_counts}")
print(f"{'=' * 60}")

# Save APNG using the apng library (proper APNG support)
print(f"\nSaving APNG...")
apng = APNG()

for i, (frame, delay) in enumerate(zip(all_frames, all_delays)):
    buf = io.BytesIO()
    frame.save(buf, format="PNG")
    buf.seek(0)
    
    png_frame = PNG.from_bytes(buf.read())
    apng.append(png_frame, delay=delay, delay_den=1000)

apng.save(OUTPUT_APNG)
print(f"  ✓ APNG saved: {OUTPUT_APNG}")
print(f"  Size: {os.path.getsize(OUTPUT_APNG) / 1024:.1f} KB")

# Save GIF (backup)
print(f"\nSaving GIF backup...")
gif_frames = [f.convert("P", palette=Image.ADAPTIVE, colors=255) for f in all_frames]
gif_frames[0].save(
    OUTPUT_GIF,
    save_all=True,
    append_images=gif_frames[1:],
    duration=all_delays,
    loop=0,
    disposal=2,
    transparency=0
)
print(f"  ✓ GIF saved: {OUTPUT_GIF}")
print(f"  Size: {os.path.getsize(OUTPUT_GIF) / 1024:.1f} KB")

print(f"\n{'=' * 60}")
print("COMPLETE!")
print(f"{'=' * 60}")