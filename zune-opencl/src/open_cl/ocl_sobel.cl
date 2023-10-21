
__kernel void SobelFilterU8(
	global uchar* inputImage,
	global uchar* outputImage,
    const int width,
    const int height
	)
{
	// This is the currently focused pixel and is the output pixel
	// location
	int2 ImageCoordinate = (int2)(get_global_id(0), get_global_id(1));

	if (ImageCoordinate.x < width  && ImageCoordinate.y < height)
    {
        // clamp to ensure that reads and writes never go out of place
        int x = min(max(ImageCoordinate.x,1),width-1);
        int y = min(max(ImageCoordinate.y,1),height-1);

		// Read the 8 pixels around the currently focused pixel
		uint Pixel00 = (uint)inputImage[((y - 1) * width) + (x - 1)];
		uint Pixel01 = (uint)inputImage[((y - 1) * width) + (x + 0)];
		uint Pixel02 = (uint)inputImage[((y - 1) * width) + (x + 1)];

	    uint Pixel10 = (uint)inputImage[((y + 0) * width) + (x - 1)];
		uint Pixel11 = (uint)inputImage[((y + 0) * width) + (x + 0)];
		uint Pixel12 = (uint)inputImage[((y + 0) * width) + (x + 1)];
		
        uint Pixel20 = (uint)inputImage[((y + 1) * width) + (x - 1)];
		uint Pixel21 = (uint)inputImage[((y + 1) * width) + (x + 0)];
		uint Pixel22 = (uint)inputImage[((y + 1) * width) + (x + 1)];
		
		// This is equivalent to looping through the 9 pixels
		// under this convolution and applying the appropriate
		// filter, here we've already applied the filter coefficients
		// since they are static
		uint Gx = Pixel00 + (2 * Pixel10) + Pixel20 -
				  Pixel02 - (2 * Pixel12) - Pixel22;
				  
		uint Gy = Pixel00 + (2 * Pixel01) + Pixel02 -
				  Pixel20 - (2 * Pixel21) - Pixel22;

		// Compute the gradient magnitude
		uint OutColor = (uint)sqrt((float)(Gx * Gx + Gy * Gy)); // R
		
		// Write the RGB value to the output image
		outputImage[((ImageCoordinate.y + 0) * width) + (ImageCoordinate.x + 0)]= OutColor;
	}
}


__kernel void SobelFilterU16(
	global ushort* inputImage,
	global ushort* outputImage,
    const int width,
    const int height
	)
{
	int2 ImageCoordinate = (int2)(get_global_id(0), get_global_id(1));

	// Make sure we are within the image bounds
    if (ImageCoordinate.x < width  && ImageCoordinate.y < height)
   	{
         // clamp to ensure that reads and writes never go out of place
         int x = min(max(ImageCoordinate.x,1),width-1);
         int y = min(max(ImageCoordinate.y,1),height-1);

		// Read the 6 pixels around the currently focused pixel
		uint Pixel00 = (uint)inputImage[((y - 1) * width) + (x - 1)];
		uint Pixel01 = (uint)inputImage[((y - 1) * width) + (x + 0)];
		uint Pixel02 = (uint)inputImage[((y - 1) * width) + (x + 1)];

	    uint Pixel10 = (uint)inputImage[((y + 0) * width) + (x - 1)];
		uint Pixel11 = (uint)inputImage[((y + 0) * width) + (x + 0)];
		uint Pixel12 = (uint)inputImage[((y + 0) * width) + (x + 1)];

        uint Pixel20 = (uint)inputImage[((y + 1) * width) + (x - 1)];
		uint Pixel21 = (uint)inputImage[((y + 1) * width) + (x + 0)];
		uint Pixel22 = (uint)inputImage[((y + 1) * width) + (x + 1)];

		uint Gx = Pixel00 + (2 * Pixel10) + Pixel20 -
				  Pixel02 - (2 * Pixel12) - Pixel22;

		uint Gy = Pixel00 + (2 * Pixel01) + Pixel02 -
				  Pixel20 - (2 * Pixel21) - Pixel22;

		// Compute the gradient magnitude
		uint OutColor = (uint)sqrt((float)(Gx * Gx + Gy * Gy)); // R

		// Write the RGB value to the output image
		outputImage[((y + 0) * width) + (x + 0)]= OutColor;
	}
}

__kernel void SobelFilterF32(
	global float* inputImage,
	global float* outputImage,
    const int width,
    const int height
	)
{
	// This is the currently focused pixel and is the output pixel
	// location
	int2 ImageCoordinate = (int2)(get_global_id(0), get_global_id(1));

    if (ImageCoordinate.x < width  && ImageCoordinate.y < height)
   	{
        // clamp to ensure that reads and writes never go out of place
        int x = min(max(ImageCoordinate.x,1),width-1);
        int y = min(max(ImageCoordinate.y,1),height-1);

		// Read the 8 pixels around the currently focused pixel
		float Pixel00 = inputImage[((y - 1) * width) + (x - 1)];
		float Pixel01 = inputImage[((y - 1) * width) + (x + 0)];
		float Pixel02 = inputImage[((y - 1) * width) + (x + 1)];

	    float Pixel10 = inputImage[((y + 0) * width) + (x - 1)];
		float Pixel11 = inputImage[((y + 0) * width) + (x + 0)];
		float Pixel12 = inputImage[((y + 0) * width) + (x + 1)];

        float Pixel20 = inputImage[((y + 1) * width) + (x - 1)];
		float Pixel21 = inputImage[((y + 1) * width) + (x + 0)];
		float Pixel22 = inputImage[((y + 1) * width) + (x + 1)];

		// This is equivalent to looping through the 9 pixels
		// under this convolution and applying the appropriate
		// filter, here we've already applied the filter coefficients
		// since they are static
		float Gx = Pixel00 + (2 * Pixel10) + Pixel20 -
				  Pixel02 - (2 * Pixel12) - Pixel22;

		float Gy = Pixel00 + (2 * Pixel01) + Pixel02 -
				  Pixel20 - (2 * Pixel21) - Pixel22;

		// Compute the gradient magnitude
		float OutColor = sqrt((float)(Gx * Gx + Gy * Gy)); // R

		// Write the RGB value to the output image
		outputImage[((y + 0) * width) + (x + 0)]= OutColor;
	}
}