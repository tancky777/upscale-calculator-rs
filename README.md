# upscale-calculator

[![tests](https://github.com/tancky777/upscale-calculator-rs/actions/workflows/tests.yml/badge.svg)](https://github.com/tancky777/upscale-calculator-rs/actions/workflows/tests.yml)

Small, dependency-free Rust helpers for the math around image upscaling:

- scaled pixel dimensions, megapixels and aspect ratio
- the smallest upscale factor (2x, 4x, 6x, or your own list) that reaches a target size
- pixel requirements and maximum print size at a given DPI

The crate only does arithmetic. It makes no network requests and does not read or write image files.

## Install

```bash
cargo add upscale-calculator
```

## Usage

```rust
use upscale_calculator::{Dimensions, UpscaleCalculator};

fn main() -> Result<(), upscale_calculator::Error> {
    let photo = Dimensions::new(1200, 1500)?;

    assert_eq!(photo.megapixels(), 1.8);
    assert_eq!(photo.aspect_ratio(), (4, 5));
    assert_eq!(photo.scale(4.0)?.to_string(), "4800x6000");
    assert_eq!(photo.fit_within(600, 600)?.to_string(), "480x600");

    // Supports 2x, 4x and 6x by default.
    let calculator = UpscaleCalculator::default();

    // Which factor do I need for a 4K wallpaper?
    let wallpaper = Dimensions::new(3840, 2160)?;
    assert_eq!(calculator.factor_for(photo, wallpaper), Some(4.0));

    // Which factor do I need for an 8x10 inch print at 300 DPI?
    assert_eq!(calculator.pixels_for_print(8.0, 10.0, 300)?.to_string(), "2400x3000");
    assert_eq!(calculator.factor_for_print(photo, 8.0, 10.0, 300)?, Some(2.0));

    // How large can I print the original?
    let print = calculator.print_size(photo, 300)?;
    assert_eq!((print.width_in, print.height_in), (4.0, 5.0));
    assert_eq!((print.width_cm, print.height_cm), (10.16, 12.7));

    Ok(())
}
```

`factor_for` returns `Some(1.0)` when the image is already large enough and `None` when none of the supported factors is enough.

Pass your own factors if your upscaler supports a different set:

```rust
use upscale_calculator::UpscaleCalculator;

let calculator = UpscaleCalculator::new(&[2.0, 3.0, 8.0]).unwrap();
assert_eq!(calculator.factors(), &[2.0, 3.0, 8.0]);
```

## About

Maintained by [Tancky AI](https://tancky.io). The default factors match the 2x, 4x and 6x options of the [Tancky AI Image Upscaler](https://tancky.io/ai-image-upscaler), where the same calculations decide which option an image needs.

## License

MIT
