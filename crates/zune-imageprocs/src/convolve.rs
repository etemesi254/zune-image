use crate::traits::NumOps;

#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
pub fn convolve_1d<T>(
    in_channel: &[T], out_channel: &mut [T], width: usize, height: usize, weights: &[f64],
    div_by: f64, max_value: u16
) where
    T: Copy + NumOps<T>,
    f64: std::convert::From<T>
{
    let chunk_size = width * 4;
    let max_value = max_value as f64;

    let radius = weights.len();
    let inv_div = 1.0 / div_by;

    // chunk it in four to get some ILPs
    for (in_stride, out_stride) in in_channel
        .chunks_exact(chunk_size)
        .zip(out_channel.chunks_exact_mut(chunk_size))
    {
        //
        let (in1, rem) = in_stride.split_at(width);
        let (in2, rem) = rem.split_at(width);
        let (in3, in4) = rem.split_at(width);

        let (ow1, rem) = out_stride.split_at_mut(width);
        let (ow2, rem) = rem.split_at_mut(width);
        let (ow3, ow4) = rem.split_at_mut(width);

        // TODO: handle edge pixels

        for (((((((o1, o2), o3), o4), w1), w2), w3), w4) in ow1[radius..]
            .iter_mut()
            .zip(ow2[radius..].iter_mut())
            .zip(ow3[radius..].iter_mut())
            .zip(ow4[radius..].iter_mut())
            .zip(in1.windows(radius))
            .zip(in2.windows(radius))
            .zip(in3.windows(radius))
            .zip(in4.windows(radius))
        {
            let (mut suma, mut sumb, mut sumc, mut sumd) = (0.0, 0.0, 0.0, 0.0);

            // sum them
            for ((((a, b), c), d), weight) in w1
                .iter()
                .zip(w2.iter())
                .zip(w3.iter())
                .zip(w4.iter())
                .zip(weights)
            {
                suma += f64::from(*a) * weight;
                sumb += f64::from(*b) * weight;
                sumc += f64::from(*c) * weight;
                sumd += f64::from(*d) * weight;
            }

            // divide them by div-by
            // but since division is slow,we multiply by its
            // inverse, which may be imprecise, but such is life
            suma *= inv_div;
            sumb *= inv_div;
            sumc *= inv_div;
            sumd *= inv_div;

            // clamp and write

            *o1 = T::from_f64(suma.clamp(0.0, max_value));
            *o2 = T::from_f64(sumb.clamp(0.0, max_value));
            *o3 = T::from_f64(sumc.clamp(0.0, max_value));
            *o4 = T::from_f64(sumd.clamp(0.0, max_value));
        }
    }
    // handle pixels that may not be divisible by 4.
    if height % 4 != 0
    {
        let unhanded_rows = (in_channel.len() / width) % 4;

        for (in_stride, out_stride) in in_channel
            .rchunks_exact(width)
            .zip(out_channel.rchunks_exact_mut(width))
            .take(unhanded_rows)
        {
            for (o1, rw1) in out_stride[radius..]
                .iter_mut()
                .zip(in_stride.windows(radius))
            {
                let mut sum = rw1
                    .iter()
                    .zip(weights.iter())
                    .map(|(x, weight)| f64::from(*x) * weight)
                    .sum::<f64>();

                sum *= inv_div;
                *o1 = T::from_f64(sum.clamp(0.0, max_value));
            }
        }
    }
}
