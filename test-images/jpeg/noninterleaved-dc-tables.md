# Non-interleaved progressive DC table regressions

Authored RGB color grids, encoded with libjpeg-turbo 3.1.4.1:

```sh
cjpeg -quality 90 -sample 1x1,1x1,1x1 -scans scans.txt -outfile grid.jpg grid.ppm
```

The palette is `(220,30,20)`, `(20,200,40)`, `(40,20,230)`, `(230,180,20)`,
`(20,180,220)`, `(240,240,240)`. Pixel `(x,y)` uses palette index
`(x/13 + 2*(y/11)) % 6`, with integer division. Sizes are 17×19, 83×59,
and 160×120. No third-party image content is included.

The 13-scan script is:

```text
0: 0 0 0 0;
1: 0 0 0 0;
2: 0 0 0 0;
0: 1 63 0 3;
0: 1 63 3 2;
1: 1 63 0 2;
2: 1 63 0 2;
0: 1 63 2 1;
1: 1 63 2 1;
2: 1 63 2 1;
0: 1 63 1 0;
1: 1 63 1 0;
2: 1 63 1 0;
```

The first fixture uses the encoder's chroma DC table 1. The other fixtures
relabel that DHT entry and its SOS DC selectors to table 2/3, preserving every
entropy-coded byte. Test expectations come from independent `djpeg` exports of
these exact fixtures, recorded with JPEG/PPM hashes in
`noninterleaved-dc-tables-measurements.json`. Tests use those sampled RGB values;
they do not run `djpeg`. At most two channel levels allow integer-IDCT rounding.

