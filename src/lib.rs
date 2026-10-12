#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]

use std::fmt;

const CM_PER_INCH: f64 = 2.54;

/// Reasons a calculation input is rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// Width or height is zero.
    EmptyDimensions,
    /// A scale factor is not a finite number in the accepted range.
    InvalidFactor,
    /// The list of supported upscale factors is empty.
    NoFactors,
    /// DPI is zero.
    InvalidDpi,
    /// A print side is not a finite number greater than zero.
    InvalidPrintSize,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Error::EmptyDimensions => "width and height must be at least 1 pixel",
            Error::InvalidFactor => "scale factor is out of range",
            Error::NoFactors => "at least one upscale factor is required",
            Error::InvalidDpi => "DPI must be at least 1",
            Error::InvalidPrintSize => "print size must be greater than 0",
        })
    }
}

impl std::error::Error for Error {}

/// Whether an image is wider than tall, taller than wide, or neither.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Orientation {
    Landscape,
    Portrait,
    Square,
}

/// Pixel dimensions of an image. Both sides are always at least 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Dimensions {
    width: u32,
    height: u32,
}

impl Dimensions {
    pub fn new(width: u32, height: u32) -> Result<Self, Error> {
        if width == 0 || height == 0 {
            return Err(Error::EmptyDimensions);
        }
        Ok(Self { width, height })
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn pixels(&self) -> u64 {
        u64::from(self.width) * u64::from(self.height)
    }

    /// Megapixels rounded to two decimals.
    pub fn megapixels(&self) -> f64 {
        (self.pixels() as f64 / 10_000.0).round() / 100.0
    }

    /// Reduced aspect ratio, for example `(16, 9)`.
    pub fn aspect_ratio(&self) -> (u32, u32) {
        let divisor = gcd(self.width, self.height);
        (self.width / divisor, self.height / divisor)
    }

    pub fn orientation(&self) -> Orientation {
        match self.width.cmp(&self.height) {
            std::cmp::Ordering::Greater => Orientation::Landscape,
            std::cmp::Ordering::Less => Orientation::Portrait,
            std::cmp::Ordering::Equal => Orientation::Square,
        }
    }

    /// Multiply both sides by a factor, rounding to whole pixels.
    pub fn scale(&self, factor: f64) -> Result<Self, Error> {
        if !factor.is_finite() || factor <= 0.0 {
            return Err(Error::InvalidFactor);
        }
        Ok(Self {
            width: scale_side(self.width, factor),
            height: scale_side(self.height, factor),
        })
    }

    /// Shrink to fit inside a bounding box while keeping the aspect ratio.
    /// Images that already fit are returned unchanged.
    pub fn fit_within(&self, max_width: u32, max_height: u32) -> Result<Self, Error> {
        if max_width == 0 || max_height == 0 {
            return Err(Error::EmptyDimensions);
        }
        let ratio = (f64::from(max_width) / f64::from(self.width))
            .min(f64::from(max_height) / f64::from(self.height));
        if ratio >= 1.0 {
            Ok(*self)
        } else {
            self.scale(ratio)
        }
    }

    /// Whether this image is at least as large as another on both sides.
    pub fn covers(&self, other: &Dimensions) -> bool {
        self.width >= other.width && self.height >= other.height
    }
}

impl fmt::Display for Dimensions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}x{}", self.width, self.height)
    }
}

/// Physical size of a print, in inches and centimeters, rounded to two decimals.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PrintSize {
    pub width_in: f64,
    pub height_in: f64,
    pub width_cm: f64,
    pub height_cm: f64,
}

/// Works out which upscale factor an image needs for a target size or print.
#[derive(Debug, Clone, PartialEq)]
pub struct UpscaleCalculator {
    factors: Vec<f64>,
}

impl Default for UpscaleCalculator {
    /// Supports 2x, 4x and 6x.
    fn default() -> Self {
        Self {
            factors: vec![2.0, 4.0, 6.0],
        }
    }
}

impl UpscaleCalculator {
    /// Build a calculator for the factors your upscaler supports. Each factor must be greater than 1.
    pub fn new(factors: &[f64]) -> Result<Self, Error> {
        if factors.is_empty() {
            return Err(Error::NoFactors);
        }
        if factors.iter().any(|f| !f.is_finite() || *f <= 1.0) {
            return Err(Error::InvalidFactor);
        }
        let mut factors = factors.to_vec();
        factors.sort_by(f64::total_cmp);
        factors.dedup();
        Ok(Self { factors })
    }

    /// Supported factors in ascending order.
    pub fn factors(&self) -> &[f64] {
        &self.factors
    }

    /// Exact multiplier needed for the source to cover the target on both sides.
    pub fn exact_factor(&self, source: Dimensions, target: Dimensions) -> f64 {
        (f64::from(target.width) / f64::from(source.width))
            .max(f64::from(target.height) / f64::from(source.height))
    }

    /// Smallest supported factor that makes the source cover the target.
    /// Returns `Some(1.0)` when no upscaling is needed and `None` when no supported factor is enough.
    pub fn factor_for(&self, source: Dimensions, target: Dimensions) -> Option<f64> {
        if source.covers(&target) {
            return Some(1.0);
        }
        self.factors.iter().copied().find(|factor| {
            let scaled = Dimensions {
                width: scale_side(source.width, *factor),
                height: scale_side(source.height, *factor),
            };
            scaled.covers(&target)
        })
    }

    /// Pixel dimensions required to print at a physical size.
    pub fn pixels_for_print(
        &self,
        width_inches: f64,
        height_inches: f64,
        dpi: u32,
    ) -> Result<Dimensions, Error> {
        let valid = |side: f64| side.is_finite() && side > 0.0;
        if !valid(width_inches) || !valid(height_inches) {
            return Err(Error::InvalidPrintSize);
        }
        if dpi == 0 {
            return Err(Error::InvalidDpi);
        }
        let dpi = f64::from(dpi);
        Dimensions::new(
            (width_inches * dpi).ceil() as u32,
            (height_inches * dpi).ceil() as u32,
        )
    }

    /// Largest print an image supports at a given DPI.
    pub fn print_size(&self, image: Dimensions, dpi: u32) -> Result<PrintSize, Error> {
        if dpi == 0 {
            return Err(Error::InvalidDpi);
        }
        let width_inches = f64::from(image.width) / f64::from(dpi);
        let height_inches = f64::from(image.height) / f64::from(dpi);
        Ok(PrintSize {
            width_in: round2(width_inches),
            height_in: round2(height_inches),
            width_cm: round2(width_inches * CM_PER_INCH),
            height_cm: round2(height_inches * CM_PER_INCH),
        })
    }

    /// Smallest supported factor that makes the source printable at a physical size.
    pub fn factor_for_print(
        &self,
        source: Dimensions,
        width_inches: f64,
        height_inches: f64,
        dpi: u32,
    ) -> Result<Option<f64>, Error> {
        let target = self.pixels_for_print(width_inches, height_inches, dpi)?;
        Ok(self.factor_for(source, target))
    }
}

fn scale_side(side: u32, factor: f64) -> u32 {
    ((f64::from(side) * factor).round() as u32).max(1)
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn gcd(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dims(width: u32, height: u32) -> Dimensions {
        Dimensions::new(width, height).unwrap()
    }

    #[test]
    fn describes_an_image() {
        let image = dims(1920, 1080);

        assert_eq!(image.pixels(), 2_073_600);
        assert_eq!(image.megapixels(), 2.07);
        assert_eq!(image.aspect_ratio(), (16, 9));
        assert_eq!(image.orientation(), Orientation::Landscape);
        assert_eq!(image.to_string(), "1920x1080");
    }

    #[test]
    fn orientation() {
        assert_eq!(dims(1080, 1920).orientation(), Orientation::Portrait);
        assert_eq!(dims(512, 512).orientation(), Orientation::Square);
    }

    #[test]
    fn scale_and_fit() {
        let image = dims(1920, 1080);

        assert_eq!(image.scale(2.0).unwrap(), dims(3840, 2160));
        assert_eq!(image.fit_within(960, 960).unwrap(), dims(960, 540));
        assert_eq!(image.fit_within(4000, 4000).unwrap(), image);
    }

    #[test]
    fn covers() {
        let image = dims(1920, 1080);

        assert!(image.covers(&dims(1920, 1080)));
        assert!(!image.covers(&dims(1921, 1080)));
    }

    #[test]
    fn rejects_invalid_input() {
        assert_eq!(Dimensions::new(0, 10), Err(Error::EmptyDimensions));
        assert_eq!(dims(10, 10).scale(0.0), Err(Error::InvalidFactor));
        assert_eq!(dims(10, 10).scale(f64::NAN), Err(Error::InvalidFactor));
        assert_eq!(UpscaleCalculator::new(&[]), Err(Error::NoFactors));
        assert_eq!(
            UpscaleCalculator::new(&[1.0, 2.0]),
            Err(Error::InvalidFactor)
        );

        let calculator = UpscaleCalculator::default();
        assert_eq!(
            calculator.print_size(dims(100, 100), 0),
            Err(Error::InvalidDpi)
        );
        assert_eq!(
            calculator.pixels_for_print(0.0, 10.0, 300),
            Err(Error::InvalidPrintSize)
        );
    }

    #[test]
    fn factors_are_sorted_and_deduplicated() {
        let calculator = UpscaleCalculator::new(&[4.0, 2.0, 2.0, 8.0]).unwrap();

        assert_eq!(calculator.factors(), &[2.0, 4.0, 8.0]);
    }

    #[test]
    fn picks_the_smallest_sufficient_factor() {
        let calculator = UpscaleCalculator::default();
        let source = dims(1000, 800);

        assert_eq!(calculator.exact_factor(source, dims(3000, 2000)), 3.0);
        assert_eq!(calculator.factor_for(source, dims(3000, 2000)), Some(4.0));
        assert_eq!(calculator.factor_for(source, dims(2000, 1600)), Some(2.0));
        assert_eq!(calculator.factor_for(source, dims(500, 500)), Some(1.0));
        assert_eq!(calculator.factor_for(source, dims(10_000, 10_000)), None);
    }

    #[test]
    fn print_math() {
        let calculator = UpscaleCalculator::default();

        assert_eq!(
            calculator.pixels_for_print(8.0, 10.0, 300).unwrap(),
            dims(2400, 3000)
        );
        assert_eq!(
            calculator.pixels_for_print(8.0, 10.0, 150).unwrap(),
            dims(1200, 1500)
        );
        assert_eq!(
            calculator.print_size(dims(3000, 2400), 300).unwrap(),
            PrintSize {
                width_in: 10.0,
                height_in: 8.0,
                width_cm: 25.4,
                height_cm: 20.32,
            }
        );
        assert_eq!(
            calculator
                .factor_for_print(dims(1200, 1500), 8.0, 10.0, 300)
                .unwrap(),
            Some(2.0)
        );
    }
}
