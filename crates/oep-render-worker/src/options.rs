//! Render options: resolution and fidelity parameterization.
//!
//! The P9-M1 job spec carries these as executor args on the `render` kind
//! (see `docs/P9-M1/job-spec.md`): `--resolution WxH` and `--fidelity`.
//! `Fidelity` is a real, testable quality knob: higher fidelity renders at a
//! supersampled internal resolution and box-downsamples, so edges are
//! anti-aliased and the output pixels differ from a low-fidelity render.

use std::fmt;
use std::str::FromStr;

/// A 2D image resolution expressed as `WIDTHxHEIGHT` (e.g. `1920x1080`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resolution {
    /// Pixel width.
    pub width: u32,
    /// Pixel height.
    pub height: u32,
}

impl Default for Resolution {
    fn default() -> Self {
        Self {
            width: 640,
            height: 480,
        }
    }
}

impl FromStr for Resolution {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (w, h) = s
            .split_once('x')
            .ok_or_else(|| format!("resolution must be WxH, got {s:?}"))?;
        let width: u32 = w
            .trim()
            .parse()
            .map_err(|_| format!("bad width in {s:?}"))?;
        let height: u32 = h
            .trim()
            .parse()
            .map_err(|_| format!("bad height in {s:?}"))?;
        if width == 0 || height == 0 {
            return Err(format!("resolution must be positive, got {s:?}"));
        }
        Ok(Self { width, height })
    }
}

impl fmt::Display for Resolution {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}x{}", self.width, self.height)
    }
}

/// Render fidelity: the supersample anti-aliasing level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Fidelity {
    /// No supersampling (1x). Fastest, sharpest aliasing.
    Low,
    /// 2x supersampling then box-downsample (2x2 average per output pixel).
    #[default]
    Medium,
    /// 4x supersampling then box-downsample (4x4 average per output pixel).
    High,
}

impl Fidelity {
    /// Supersample factor applied to the internal render resolution.
    pub fn factor(self) -> u32 {
        match self {
            Self::Low => 1,
            Self::Medium => 2,
            Self::High => 4,
        }
    }

    /// Canonical names in the fixed order the registration advertises.
    pub const ALL: [&'static str; 3] = ["low", "medium", "high"];
}

impl FromStr for Fidelity {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "low" => Ok(Self::Low),
            "medium" | "med" => Ok(Self::Medium),
            "high" => Ok(Self::High),
            other => Err(format!(
                "unknown fidelity {other:?}; expected one of {}",
                Fidelity::ALL.join(", ")
            )),
        }
    }
}

/// Complete render options for one job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RenderOptions {
    /// Output resolution.
    pub resolution: Resolution,
    /// Fidelity (supersample) level.
    pub fidelity: Fidelity,
}

impl RenderOptions {
    /// New options at a resolution and fidelity.
    pub fn new(resolution: Resolution, fidelity: Fidelity) -> Self {
        Self {
            resolution,
            fidelity,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_wxh_parses() {
        let r: Resolution = "1920x1080".parse().unwrap();
        assert_eq!((r.width, r.height), (1920, 1080));
    }

    #[test]
    fn resolve_rejects_malformed() {
        assert!("640".parse::<Resolution>().is_err());
        assert!("0x480".parse::<Resolution>().is_err());
        assert!("axb".parse::<Resolution>().is_err());
    }

    #[test]
    fn fidelity_factors_and_names() {
        assert_eq!(Fidelity::Low.factor(), 1);
        assert_eq!(Fidelity::Medium.factor(), 2);
        assert_eq!(Fidelity::High.factor(), 4);
        assert_eq!(Fidelity::ALL, ["low", "medium", "high"]);
        assert_eq!("HIGH".parse::<Fidelity>().unwrap(), Fidelity::High);
    }
}
