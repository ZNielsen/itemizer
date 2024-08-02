# Itemizer !! WIP - currently non-functional !!

Itemizer is a receipt scanner tool designed to give insights into where our
grocery budget is disappearing to. It will scan a directory for images, pull out
the individual items on the receipt, then log them to a sqlite database for
later use.

## Setup

Requires the following environment variables to be set:
- `ITEMIZER_IMAGE_DIR` - The input directory
- `ITEMIZER_UPSCALED_IMAGE_DIR` - Where to store upscaled images
- `ITEMIZER_IMAGE_DONE_FILE` - A file to track which receipts are done to avoid duplicate effort/entries
- `ITEMIZER_RULES_FILE` - The 'rules' file, see below
- `ITEMIZER_DB` - Location for the database file, when using database mode
- `ITEMIZER_PURCHASES_FILE` - Location for the purchases file, when using file mode

The `ITEMIZER_RULES_FILE` is structured as blocks of text describing a store item and its properties.
The order and type of each item is important:

- UPC code - integer
- Description, as it appears on the receipt - string>
- Name, as the user would like to refer to the product - string>
- Tags, (optional) for sorting, comma separated - string>

Example:
```
4093
ONION YLW CO
Onions
veggies, produce

4164501
KS SPARKLING
Bubbly Water

1326
COCONUT STRIPS
Coconut Strips
snacks

7055280188
WINCO TST PSTRY
Pop Tarts
EXCLUDE
```

In the example, `Bubbly Water` does not have any tags, while `Onions` is tagged
with both `veggies` and `produce`; all instances of `Onions` will be counted
towards the totals of both lists. `Pop Tarts` has been tagged with the special
`EXCLUDE` tag; it will not be included in any list, including the top level
monthly total.

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
