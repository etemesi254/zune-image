### Things you can do.

1. Convert an image with an ICC profile to correct format.
   
    Some images require post-processing of the ICC profile to match with what a browser can show you.

    ```shell
    zune  -i "{IMAGE-WITH-ICC}"  --color-transform rgb -o ./hello.png --trace
    ```