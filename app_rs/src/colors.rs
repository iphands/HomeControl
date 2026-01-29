//! Predefined RGB color constants for LED strip animations.
//!
//! These constants provide commonly used colors as `[u8; 3]` arrays
//! in RGB format for convenient use in animation modes.

/// Red color: `[255, 0, 0]`
pub const RED: [u8; 3] = [255, 0, 0];

/// Green color: `[0, 255, 0]`
pub const GREEN: [u8; 3] = [0, 255, 0];

/// Blue color: `[0, 0, 255]`
pub const BLUE: [u8; 3] = [0, 0, 255];

/// Yellow color: `[255, 255, 0]`
pub const YELLOW: [u8; 3] = [255, 255, 0];

/// Purple/Magenta color: `[255, 0, 255]`
pub const PURPLE: [u8; 3] = [255, 0, 255];

/// Cyan color: `[0, 255, 255]`
#[allow(dead_code)]
pub const CYAN: [u8; 3] = [0, 255, 255];

/// White color: `[255, 255, 255]`
pub const WHITE: [u8; 3] = [255, 255, 255];

/// Black/Off color: `[0, 0, 0]`
pub const BLACK: [u8; 3] = [0, 0, 0];

/// Orange color: `[255, 165, 0]`
#[allow(dead_code)]
pub const ORANGE: [u8; 3] = [255, 165, 0];

/// Pink color: `[255, 192, 203]`
#[allow(dead_code)]
pub const PINK: [u8; 3] = [255, 192, 203];

/// Collection of all available colors for random selection.
#[allow(dead_code)]
pub const ALL_COLORS: &[[u8; 3]] = &[RED, GREEN, BLUE, YELLOW, PURPLE, CYAN, WHITE, ORANGE, PINK];

/// Christmas-themed colors.
pub const CHRISTMAS_COLORS: &[[u8; 3]] = &[RED, GREEN];

/// Mardi Gras themed colors.
pub const MARDI_GRAS_COLORS: &[[u8; 3]] = &[PURPLE, GREEN, YELLOW];

/// RGB cycle colors.
pub const RGB_COLORS: &[[u8; 3]] = &[RED, GREEN, BLUE];
