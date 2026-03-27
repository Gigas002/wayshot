#[cfg(feature = "selector")]
use crate::utils::get_region_area;
use dialoguer::{FuzzySelect, theme::ColorfulTheme};
use eyre::{Result, bail};
use libwayshot::{CaptureBufferBackend, WayshotConnection};

/// Describes what was captured, used to build the notification body.
#[derive(Debug, Clone)]
#[cfg_attr(not(feature = "notifications"), allow(dead_code))]
pub enum ShotResult {
    Output { name: String },
    Toplevel { name: String },
    Area,
    All,
}

/// How the screenshot target is determined.
pub enum CaptureMode {
    /// Interactive area/region selection via waysip.
    #[cfg(feature = "selector")]
    Geometry,
    /// A region from an external tool (e.g. slurp), parsed from "x,y widthxheight".
    GeometryRegion(libwayshot::LogicalRegion),
    /// A specific toplevel window by its id+title string.
    Toplevel(String),
    /// Interactive fuzzy-select from active toplevel windows.
    ChooseToplevel,
    /// A named output/display.
    Output(String),
    /// Interactive fuzzy-select from available outputs.
    ChooseOutput,
    /// Every connected output at once.
    All,
}

/// Capture a screenshot according to `mode` and [`CaptureBufferBackend`].
pub fn capture(
    conn: &WayshotConnection,
    mode: &CaptureMode,
    cursor: bool,
    #[cfg_attr(not(feature = "selector"), allow(unused_variables))] freeze: bool,
    backend: CaptureBufferBackend,
) -> Result<(image::DynamicImage, ShotResult)> {
    match mode {
        #[cfg(feature = "selector")]
        CaptureMode::Geometry => capture_geometry(conn, cursor, freeze, backend),
        CaptureMode::GeometryRegion(region) => {
            let image = conn.screenshot_with_backend(*region, cursor, backend)?;
            Ok((image, ShotResult::Area))
        }
        CaptureMode::Toplevel(name) => capture_toplevel_by_name(conn, name, cursor, backend),
        CaptureMode::ChooseToplevel => capture_toplevel_interactive(conn, cursor, backend),
        CaptureMode::Output(name) => capture_output_by_name(conn, name, cursor, backend),
        CaptureMode::ChooseOutput => capture_output_interactive(conn, cursor, backend),
        CaptureMode::All => Ok((
            conn.screenshot_all_with_backend(cursor, backend)?,
            ShotResult::All,
        )),
    }
}

/// Capture an interactively selected screen region.
#[cfg(feature = "selector")]
fn capture_geometry(
    conn: &WayshotConnection,
    cursor: bool,
    freeze: bool,
    backend: CaptureBufferBackend,
) -> Result<(image::DynamicImage, ShotResult)> {
    let image = if freeze {
        conn.screenshot_freeze_with_backend(
            |w_conn| get_region_area(w_conn).map_err(libwayshot::Error::FreezeCallbackError),
            cursor,
            backend,
        )?
    } else {
        let region = get_region_area(conn).map_err(|e| eyre::eyre!("{e}"))?;
        conn.screenshot_with_backend(region, cursor, backend)?
    };
    Ok((image, ShotResult::Area))
}

fn capture_toplevel_by_name(
    conn: &WayshotConnection,
    name: &str,
    cursor: bool,
    backend: CaptureBufferBackend,
) -> Result<(image::DynamicImage, ShotResult)> {
    let toplevels = conn.get_all_toplevels();
    let toplevel = toplevels
        .iter()
        .filter(|t| t.active)
        .find(|t| t.id_and_title() == name)
        .ok_or_else(|| eyre::eyre!("No toplevel window matched '{name}'"))?;
    Ok((
        conn.screenshot_toplevel_with_backend(toplevel, cursor, backend)?,
        ShotResult::Toplevel {
            name: name.to_string(),
        },
    ))
}

fn capture_toplevel_interactive(
    conn: &WayshotConnection,
    cursor: bool,
    backend: CaptureBufferBackend,
) -> Result<(image::DynamicImage, ShotResult)> {
    let toplevels = conn.get_all_toplevels();
    let active: Vec<_> = toplevels.iter().filter(|t| t.active).collect();
    if active.is_empty() {
        bail!("No active toplevel windows found!");
    }
    let names: Vec<String> = active.iter().map(|t| t.id_and_title()).collect();
    let idx = fuzzy_select(&names).ok_or_else(|| eyre::eyre!("No toplevel window selected!"))?;
    Ok((
        conn.screenshot_toplevel_with_backend(active[idx], cursor, backend)?,
        ShotResult::Toplevel {
            name: names[idx].clone(),
        },
    ))
}

fn capture_output_by_name(
    conn: &WayshotConnection,
    name: &str,
    cursor: bool,
    backend: CaptureBufferBackend,
) -> Result<(image::DynamicImage, ShotResult)> {
    let outputs = conn.get_all_outputs();
    let output = outputs
        .iter()
        .find(|o| o.name == name)
        .ok_or_else(|| eyre::eyre!("No output named '{name}' found"))?;
    Ok((
        conn.screenshot_single_output_with_backend(output, cursor, backend)?,
        ShotResult::Output {
            name: name.to_string(),
        },
    ))
}

fn capture_output_interactive(
    conn: &WayshotConnection,
    cursor: bool,
    backend: CaptureBufferBackend,
) -> Result<(image::DynamicImage, ShotResult)> {
    let outputs = conn.get_all_outputs();
    let names: Vec<&str> = outputs.iter().map(|o| o.name.as_str()).collect();
    let idx = fuzzy_select(&names).ok_or_else(|| eyre::eyre!("No output selected!"))?;
    Ok((
        conn.screenshot_single_output_with_backend(&outputs[idx], cursor, backend)?,
        ShotResult::Output {
            name: names[idx].to_string(),
        },
    ))
}

fn fuzzy_select<T: ToString + std::fmt::Display>(items: &[T]) -> Option<usize> {
    FuzzySelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Choose Screen")
        .default(0)
        .items(items)
        .interact()
        .ok()
}
