# Itemizer

Itemizer is a receipt scanner tool designed to give insights into where our
grocery budget is disappearing to. It will scan a directory for images, pull out
the individual items on the receipt, then log them to a sqlite database for
later use.

## Setup

Requires the following environment variables to be set:
- `ITEMIZER_IMAGE_DIR` - The input directory
- `ITEMIZER_UPSCALED_IMAGE_DIR` - Where to store upscaled images
- `ITEMIZER_IMAGE_DONE_FILE` - A file to track which receipts are done to avoid duplicate effort/entries
- `ITEMIZER_ITEMS_FILE` - The 'rules' file, see below

The `ITEMIZER_ITEMS_FILE` (TODO: explanation of items file)

## Code Stuff
### License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
