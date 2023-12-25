## Adding a new test to the repo

This walkthrough takes one through steps it takes to add a simple test to the library.

### 1. Add image/images to test-images

`test-images` directory is the repo that contains sample images that help test for regressions and correctness.

It contains a set of small/medium-sized images that stress certain parts of the decoder, so in case we have a
regression we don't want to occur, we add the image to `test-images`.

The structure is `test-images->format->[images]` for ease of use.

- Remember to add the images to VCS

### 2. Add the file details to `tests/tests/[{format}.json]`

E.g there is a `jpeg.json` which contains information about jpeg files.

The format is simple

| Field      | Status    | Explanation                                                                 |
|------------|-----------|-----------------------------------------------------------------------------|
| name       | Mandatory | File name with extension                                                    |
| hash       | Mandatory | xxhash(seed of `0`) result of the decoded result still not in planar format |
| colorspace | Optional  | Expected colorspace output of the image, only used by `zune-jpeg` currently |
| comment    | Optional  | A reason why the file exists, useful for debugging purposes                 |                

Here is an example of such a file description

```json
{
  "name": "weid_sampling_factors.jpg",
  "hash": 290923794124024565485763288568005295883,
  "comment": "Non normal sampling factors, h2v1 for all components",
  "colorspace": "rgb"
}
```

Each entry is part of an array, so it should be inside `[]`

#### Tip: Incase you don't know the hash and you have ensured the output is correct(matches other libraries)

- Run the testing once, once you get the error, copy the hash printed to stdout

### 3. Run decoder

Ensure nothing is failing.

### 4. Profit.

Now we can easily track regressions 