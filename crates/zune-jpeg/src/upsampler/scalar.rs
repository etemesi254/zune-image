pub fn upsample_horizontal(
    input: &[i16], _ref: &mut [i16], _scratch: &mut [i16], output: &mut [i16],
)
{
    assert_eq!(
        input.len() * 2,
        output.len(),
        "Input length is not half the size of the output length"
    );
    assert!(
        output.len() > 4 && input.len() > 2,
        "Too Short of a vector, cannot upsample"
    );

    output[0] = input[0];
    output[1] = (input[0] * 3 + input[1] + 2) >> 2;

    // This code is written for speed and not readability
    //
    // The readable code is
    //
    //      for i in 1..input.len() - 1{
    //         let sample = 3 * input[i] + 2;
    //         out[i * 2] = (sample + input[i - 1]) >> 2;
    //         out[i * 2 + 1] = (sample + input[i + 1]) >> 2;
    //     }
    //
    // The output of a pixel is determined by it's surrounding neighbours but we attach more weight to it's nearest
    // neighbour (input[i]) than to the next nearest neighbour.

    for (output_window, input_window) in output[2..].chunks_exact_mut(2).zip(input.windows(3))
    {
        let sample = 3 * input_window[1] + 2;

        output_window[0] = (sample + input_window[0]) >> 2;
        output_window[1] = (sample + input_window[2]) >> 2;
    }
    // Get lengths
    let out_len = output.len() - 2;
    let input_len = input.len() - 2;

    // slice the output vector
    let f_out = &mut output[out_len..];
    let i_last = &input[input_len..];

    // write out manually..
    f_out[0] = (3 * i_last[0] + i_last[1] + 2) >> 2;
    f_out[1] = i_last[1];
}
pub fn upsample_vertical(
    input: &[i16], in_ref: &mut [i16], _scratch_space: &mut [i16], output: &mut [i16],
)
{
    let middle = output.len() / 2;

    let (out_top, out_bottom) = output.split_at_mut(middle);

    for (((near, far), ot), ob) in in_ref.iter().zip(input.iter()).zip(out_top).zip(out_bottom)
    {
        *ot = (((3 * near) + 2) + far) >> 2;
        *ob = (((3 * far) + 2) + near) >> 2;
    }
    // copy input to ref to be used for the next row
    in_ref.copy_from_slice(input);
}

pub fn upsample_hv(input: &[i16], in_ref: &mut [i16], scratch_space: &mut [i16], output: &mut [i16])
{
    let mut t = [0];
    upsample_vertical(input, in_ref, &mut t, scratch_space);
    upsample_horizontal(scratch_space, in_ref, &mut t, output);
}
