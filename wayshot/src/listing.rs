use serde::Serialize;

// Not using libwayshot::region::{Position, Size} because they don't derive Serialize
// Not deriving Serialize in libwayshot to keep serde dep out of there
#[derive(Serialize)]
pub struct PositionInfo {
    /// X coordinate.
    pub x: i32,
    /// Y coordinate.
    pub y: i32,
}

#[derive(Serialize)]
pub struct SizeInfo<T = u32> {
    /// Width.
    pub width: T,
    /// Height.
    pub height: T,
}

#[derive(Serialize)]
pub struct DisplayInfo {
    pub name: String,
    pub description: String,
    pub size: SizeInfo,
    pub logical_size: SizeInfo,
    pub position: PositionInfo,
}

#[derive(Serialize)]
pub struct ToplevelInfo {
    pub title: String,
    pub app_id: String,
    pub identifier: String,
}
