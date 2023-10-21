__kernel void brighten_u8(
          global uchar* inputImage,
          const uchar num,
          const int width,
          const int height)
{
    int x = get_global_id(0);
    int y = get_global_id(1);

    if (x < width && y < height)
    {
        int index = y * width + x;
        uchar pixel = inputImage[index];
        uchar result = clamp(pixel + num, 0, 255);

        inputImage[index] = result;
    }
}
__kernel void brighten_u16(
          global ushort* inputImage,
          const ushort num,
          const int width,
          const int height)
{
    int x = get_global_id(0);
    int y = get_global_id(1);

    if (x < width && y < height)
    {
        int index = y * width + x;
        ushort pixel = inputImage[index];
        ushort result = clamp(pixel + num, 0, 65535);

        inputImage[index] = result;
    }
}
__kernel void brighten_f32(
          global float* inputImage,
          const float num,
          const int width,
          const int height)
{
    int x = get_global_id(0);
    int y = get_global_id(1);

    if (x < width && y < height)
    {
        int index = y * width + x;
        float pixel = inputImage[index];
        float result = clamp(pixel + num, 0.0f, 1.0f);

        inputImage[index] = result;
    }
}