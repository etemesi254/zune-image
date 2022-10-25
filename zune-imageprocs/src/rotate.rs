use log::warn;

use crate::flip::flip;

pub fn rotate(angle: u16, in_image: &[u8], out_image: &mut [u8])
{
    let angle = angle % 360;
    if angle == 180
    {
        rotate_180(in_image, out_image);
    }
}
fn rotate_180(in_image: &[u8], out_image: &mut [u8])
{
    // rotate 180 is the same as flip, so use that
    // copy to dest
    out_image.copy_from_slice(in_image);
    // flip that.
    flip(out_image);
}

fn rotate_90(in_image: &[u8], out_image: &mut [u8], width: usize, height: usize)
{
    // a 90 degree rotation is a bit cache unfriendly,
    // since widths become heights, but we can still optimize it
    //                   ┌──────┐
    //┌─────────┐        │ ───► │
    //│ ▲       │        │ 90   │
    //│ │       │        │      │
    //└─┴───────┘        │      │
    //                   └──────┘
    //
    // The lower pixel becomes the top most pixel
    //
}
