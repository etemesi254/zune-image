#!/bin/bash
# test_composite.sh — generate test images and exercise every zune composite method
# Usage: ./test_composite.sh [path/to/zune]
# Requires: ImageMagick (magick), zune on PATH or passed as $1

set -euo pipefail

ZUNE="${1:-zune}"
WORK="$(mktemp -d)"
PASS=0
FAIL=0

cleanup() { rm -rf "$WORK"; }
trap cleanup EXIT

log()  { printf '  %s\n' "$*"; }
ok()   { printf '  \033[32m✓\033[0m  %s\n' "$*"; PASS=$((PASS+1)); }
fail() { printf '  \033[31m✗\033[0m  %s\n' "$*"; FAIL=$((FAIL+1)); }

# ── prerequisites ─────────────────────────────────────────────────────────────

check_deps() {
    for cmd in magick "$ZUNE"; do
        if ! command -v "$cmd" &>/dev/null; then
            echo "ERROR: '$cmd' not found. Install ImageMagick or pass zune path as \$1." >&2
            exit 1
        fi
    done
}

# ── image generation ──────────────────────────────────────────────────────────

make_images() {
    echo
    echo "Generating test images in $WORK …"

    # dst: solid blue square on transparent canvas, RGBA 256x256
    # -extent 256x256 pins the canvas so ImageMagick doesn't crop to the
    # bounding box of the drawn content — zune's from_u8 requires exact dimensions
    magick -size 256x256 xc:none \
        -type TrueColorAlpha -alpha set \
        -fill "rgba(0,0,255,255)" \
        -draw "rectangle 20,68 148,188" \
        -extent 256x256 \
        -define png:color-type=6 \
        -depth 16 \
        "$WORK/dst_shape.png"

    # src: solid red circle on transparent canvas, RGBA 256x256
    magick -size 256x256 xc:none \
        -type TrueColorAlpha -alpha set \
        -fill "rgba(255,0,0,255)" \
        -draw "circle 148,128 240,128" \
        -extent 256x256 \
        -define png:color-type=6 \
        -depth 16 \
        "$WORK/src_shape.png"

    # Crossing gradients — ideal for Multiply and Screen.
    # -depth 8 ensures plain 8-bit RGB with no alpha so colorspace matches.
    magick -size 256x256 gradient:black-white -rotate 90 -depth 16 "$WORK/grad_h.png"
    magick -size 256x256 gradient:black-white            -depth 16 "$WORK/grad_v.png"

    # Half-transparent versions of each shape — exercises the Over alpha formula
    magick "$WORK/dst_shape.png" -channel A -evaluate multiply 0.5 +channel \
        -extent 256x256 \
        -depth 16 \
        -type TrueColorAlpha -alpha set \
        "$WORK/dst_shape_50pct.png"
    magick "$WORK/src_shape.png" -channel A -evaluate multiply 0.5 +channel \
        -extent 256x256 \
        -depth 16 \
        -type TrueColorAlpha -alpha set \
        "$WORK/src_shape_50pct.png"

    # Verify every generated image has the expected dimensions — catches silent
    # crop/resize surprises from ImageMagick before zune ever sees the files.
    verify_dims() {
        local file="$1" expected="$2"
        local actual
        actual=$(magick identify -format "%wx%h" "$file")
        if [ "$actual" != "$expected" ]; then
            echo "ERROR: $file is ${actual}, expected ${expected}" >&2
            exit 1
        fi
    }
    for f in dst_shape.png src_shape.png dst_shape_50pct.png src_shape_50pct.png \
              grad_h.png grad_v.png; do
        verify_dims "$WORK/$f" "256x256"
    done

    log "dst_shape.png       — blue square, fully opaque, RGBA"
    log "src_shape.png       — red circle,  fully opaque, RGBA"
    log "dst_shape_50pct.png — blue square, 50 % alpha,   RGBA"
    log "src_shape_50pct.png — red circle,  50 % alpha,   RGBA"
    log "grad_h.png          — horizontal gradient, RGB"
    log "grad_v.png          — vertical gradient,   RGB"
}

# ── reference generation (ImageMagick) ────────────────────────────────────────

make_references() {
    echo
    echo "Generating ImageMagick reference outputs …"

    # Map zune method names to ImageMagick compose operator names
    im_op() {
        case "$1" in
            Over)     echo "over"     ;;
            Src)      echo "src"      ;;
            Dst)      echo "dst"      ;;
            DstIn)    echo "dstin"    ;;
            DstOut)   echo "dstout"   ;;
            SrcIn)    echo "srcin"    ;;
            SrcOut)   echo "srcout"   ;;
            Xor)      echo "xor"      ;;
            Multiply) echo "multiply" ;;
            Screen)   echo "screen"   ;;
        esac
    }

    for method in Over Src Dst DstIn DstOut SrcIn SrcOut Xor Multiply Screen; do
        # Porter-Duff group: shape pair
        magick "$WORK/dst_shape.png" "$WORK/src_shape.png" \
            -compose "$(im_op "$method")" -composite \
            "$WORK/ref_shape_${method}.png" 2>/dev/null
        log "ref_shape_${method}.png"
    done

    # Blend modes on gradients
    for method in Multiply Screen; do
        magick "$WORK/grad_h.png" "$WORK/grad_v.png" \
            -compose "$(im_op "$method")" -composite \
            "$WORK/ref_grad_${method}.png" 2>/dev/null
        log "ref_grad_${method}.png"
    done

    # Over with semi-transparent inputs
    magick "$WORK/dst_shape_50pct.png" "$WORK/src_shape_50pct.png" \
        -compose over -composite \
        "$WORK/ref_shape_Over_50pct.png" 2>/dev/null
    log "ref_shape_Over_50pct.png"
}

# ── zune runner ───────────────────────────────────────────────────────────────

run_zune() {
    local method="$1" dst="$2" src="$3" out="$4"
    "$ZUNE" -i "$dst" -i "$src" --depth 16 --composite "$method" --geometry 0,0 -o "$out"
}

# ── pixel-level comparison ────────────────────────────────────────────────────

# Returns the number of pixels that differ by more than $fuzz (0–255).
pixel_diff() {
    local ref="$1" out="$2" fuzz="${3:-1}"
    # AE = absolute error count; fuzz allows ±1 for integer rounding differences
    magick "$ref" "$out" \
        -fuzz "${fuzz}" \
        -metric AE \
        -compare -format "%[distortion]" info:
}

# ── test cases ────────────────────────────────────────────────────────────────

run_tests() {
    echo
    echo "Running composite tests …"
    echo
    # set -x

    # ── Porter-Duff operators on opaque shapes ────────────────────────────────
    echo "Porter-Duff (opaque shapes):"

    for method in Over Src Dst DstIn DstOut SrcIn SrcOut Xor; do
        out="$WORK/out_shape_${method}.png"
        ref="$WORK/ref_shape_${method}.png"

        run_zune "$method" "$WORK/dst_shape.png" "$WORK/src_shape.png" "$out"

        diff=$(pixel_diff "$ref" "$out" 1)
        diff_int=$(printf "%.0f" "$diff")
        if [ "$diff_int" -eq 0 ]; then
            ok "$method — 0 differing pixels"
        else
            fail "$method — ${diff} pixels differ from ImageMagick reference"
            log "  inspect: magick $ref $out -compose difference -composite diff_${method}.png"
        fi
    done

    echo
    echo "Over (semi-transparent inputs):"

    out="$WORK/out_shape_Over_50pct.png"
    ref="$WORK/ref_shape_Over_50pct.png"
    run_zune "Over" "$WORK/dst_shape_50pct.png" "$WORK/src_shape_50pct.png" "$out"
    diff=$(pixel_diff "$ref" "$out" 1)
    diff_int=$(printf "%.0f" "$diff")

    if [ "$diff_int" -eq 0 ]; then
        ok "Over 50% alpha — 0 differing pixels"
    else
        fail "Over 50% alpha — ${diff} pixels differ"
    fi

    # ── Blend modes on crossing gradients ────────────────────────────────────
    echo
    echo "Blend modes (crossing gradients):"

    for method in Multiply Screen; do
        out="$WORK/out_grad_${method}.png"
        ref="$WORK/ref_grad_${method}.png"

        run_zune "$method" "$WORK/grad_h.png" "$WORK/grad_v.png" "$out"

        diff=$(pixel_diff "$ref" "$out" 1)
        diff_int=$(printf "%.0f" "$diff")

        if [ "$diff_int" -eq 0 ]; then
            ok "$method — 0 differing pixels"
        else
            fail "$method — ${diff_int} pixels differ from ImageMagick reference"
        fi
    done

    # ── Visual sanity checks (no reference, just assert output exists + non-trivial) ──
    echo
    echo "Visual sanity checks:"

    # Dst is a no-op — output must be identical to the dst input
    out="$WORK/out_dst_noop.png"
    run_zune "Dst" "$WORK/dst_shape.png" "$WORK/src_shape.png" "$out"
    diff=$(pixel_diff "$WORK/dst_shape.png" "$out" 0)
    diff_int=$(printf "%.0f" "$diff")

    if [ "$diff_int" -eq 0 ]; then
        ok "Dst is a no-op (output == dst input)"
    else
        fail "Dst changed ${diff} pixels — should be no-op"
    fi

    # DstIn with fully opaque src must equal dst
    out="$WORK/out_dstin_opaque.png"
    run_zune "DstIn" "$WORK/dst_shape.png" "$WORK/src_shape.png" "$out"
    diff=$(pixel_diff "$WORK/ref_shape_DstIn.png" "$out" 1)
    if [ "$diff" -eq 0 ]; then
        ok "DstIn (opaque src) matches reference"
    else
        fail "DstIn (opaque src) — ${diff} pixels differ"
    fi

    # DstOut + DstIn should reconstruct the original dst (they are complementary)
    out_in="$WORK/out_dstin_comp.png"
    out_out="$WORK/out_dstout_comp.png"
    out_reconstructed="$WORK/out_reconstructed.png"
    run_zune "DstIn"  "$WORK/dst_shape.png" "$WORK/src_shape.png" "$out_in"
    run_zune "DstOut" "$WORK/dst_shape.png" "$WORK/src_shape.png" "$out_out"
    # Merge the two halves with Over; result should equal original dst
    magick "$out_out" "$out_in" -compose over -composite "$out_reconstructed"
    diff=$(pixel_diff "$WORK/dst_shape.png" "$out_reconstructed" 1)
    if [ "$diff" -eq 0 ]; then
        ok "DstIn ∪ DstOut reconstructs original dst"
    else
        fail "DstIn ∪ DstOut reconstruction — ${diff} pixels differ from original dst"
    fi

    # Screen(x, 0) == x  (black is identity for Screen)
    out="$WORK/out_screen_black.png"
    black="$WORK/black.png"
    magick -size 256x256 xc:black "$black"
    run_zune "Screen" "$WORK/grad_h.png" "$black" "$out"
    diff=$(pixel_diff "$WORK/grad_h.png" "$out" 1)
    if [ "$diff" -eq 0 ]; then
        ok "Screen(x, black) == x"
    else
        fail "Screen(x, black) — ${diff} pixels differ from identity"
    fi

    # Multiply(x, white) == x  (white is identity for Multiply)
    out="$WORK/out_multiply_white.png"
    white="$WORK/white.png"
    magick -size 256x256 xc:white "$white"
    run_zune "Multiply" "$WORK/grad_h.png" "$white" "$out"
    diff=$(pixel_diff "$WORK/grad_h.png" "$out" 1)
    if [ "$diff" -eq 0 ]; then
        ok "Multiply(x, white) == x"
    else
        fail "Multiply(x, white) — ${diff} pixels differ from identity"
    fi
}

# ── summary ───────────────────────────────────────────────────────────────────

summary() {
    local total=$((PASS+FAIL))
    echo
    echo "────────────────────────────────"
    printf '  %d / %d tests passed\n' "$PASS" "$total"
    if [ "$FAIL" -gt 0 ]; then
        echo
        echo "  To inspect failures, outputs are in:"
        echo "  $WORK  (remove the EXIT trap to keep them)"
        echo "────────────────────────────────"
        exit 1
    fi
    echo "────────────────────────────────"
}

# ── main ──────────────────────────────────────────────────────────────────────

check_deps
make_images
make_references
run_tests
summary